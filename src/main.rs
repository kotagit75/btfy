#[macro_use]
extern crate log;
extern crate simple_logger as logger;

extern crate regex;

use clap::Parser;
use log::Level;
use serde::Deserialize;
use std::{
    net::Ipv4Addr,
    str::FromStr,
    sync::{Arc, LazyLock},
};
use tokio::sync::{mpsc, watch};

use crate::{
    beacon::InMemoryBeaconCache,
    node::save_chain,
    p2p::Peer,
    state::State,
    update::{Command, Event, UpdateResult, run_effect, update},
};

pub mod api;
pub mod beacon;
pub mod blockchain;
pub mod node;
pub mod p2p;
pub mod state;
pub mod update;
pub mod util;

#[tokio::main]
async fn main() {
    logger::init_with_level(Level::Info).unwrap();

    let Some(mut state) = init_state() else {
        return;
    };
    debug!("address: {}", state.address.der);

    let (event_tx, mut event_rx) = mpsc::channel(256);
    let (state_tx, state_rx) = watch::channel(state.clone());
    init_p2p_and_api(state_rx, event_tx.clone()).await;
    let beacon_cache = Arc::new(InMemoryBeaconCache::new());

    if CONFIG.args.mining {
        let _ = event_tx.send(Command::Event(Event::MineBlock)).await;
    }
    if let Some(address) = CONFIG.args.peer.clone() {
        match Ipv4Addr::from_str(&address) {
            Ok(ip) => {
                let _ = event_tx
                    .send(Command::Event(Event::AddPeer(Peer::new(ip))))
                    .await;
            }
            Err(e) => error!("invalid ip address: {}", e),
        }
    }

    while let Some(command) = event_rx.recv().await {
        let (event, response_tx) = match command {
            Command::Event(event) => (event, None),
            Command::ApiRequest(event, response_tx) => (event, Some(response_tx)),
        };
        let previous_state = state.clone();
        let previous_chain = state.chain.clone();
        let (new_state, effect) = update(event, state, beacon_cache.as_ref()).await;
        state = new_state.clone();
        if state.chain != previous_chain {
            let _ = save_chain(&state.chain).inspect_err(|e| error!("failed to save chain: {}", e));
        }
        let _ = state_tx.send(state.clone());
        if let Some(response_tx) = response_tx {
            let _ = response_tx.send(UpdateResult {
                changed: new_state != previous_state,
                effect: effect.clone(),
            });
        }
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let events = run_effect(new_state, effect).await;
            for event in events {
                let _ = event_tx_clone.send(Command::Event(event)).await;
            }
        });
    }
}

fn init_state() -> Option<State> {
    info!("loading node key");
    let Ok(sk) = node::load_or_generate_key() else {
        error!("failed to load node key");
        return None;
    };
    info!("loading chain");
    let Ok(chain) = node::load_or_generate_chain() else {
        error!("failed to load chain");
        return None;
    };
    info!("initializing state");
    Some(state::State::new(sk, chain))
}

async fn init_p2p_and_api(state_rx: watch::Receiver<State>, event_tx: mpsc::Sender<Command>) -> () {
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        api::init_api(event_tx_clone, state_rx).await;
    });
    tokio::spawn(async move {
        p2p::init_p2p(event_tx).await;
    });
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Whether to mine blocks
    #[arg(short, long)]
    mining: bool,

    /// The IP address to add to the peer list
    #[arg(short, long)]
    peer: Option<String>,

    /// The port to listen on for the API
    #[arg(short, long, default_value = "8080")]
    api_port: u16,

    /// The timeout for API requests in seconds
    #[arg(short, long, default_value = "5")]
    beacon_timeout: u64,

    /// Beacon provider command to run over stdio
    #[arg(long = "beacon-cmd", num_args = 1.., value_name = "CMD")]
    beacon_cmd: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct InternalConfig {
    pub p2p_port: u16,
    pub vdf_difficulty: u64,
}
pub struct Config {
    args: Args,
    internal_config: InternalConfig,
}

const INTERNAL_CONFIG_JSON: &str = include_str!("config.json");
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let args = Args::parse();

    let internal_config: InternalConfig = match serde_json::from_str(INTERNAL_CONFIG_JSON) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("failed to parse internal config: {}", e);
            InternalConfig {
                p2p_port: 62697,
                vdf_difficulty: 5295676,
            }
        }
    };
    Config {
        args,
        internal_config,
    }
});
