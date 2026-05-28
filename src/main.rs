#[macro_use]
extern crate log;
extern crate simple_logger as logger;

extern crate regex;

use clap::Parser;
use log::Level;
use std::{net::Ipv4Addr, str::FromStr, sync::Arc};
use tokio::sync::{mpsc, watch};

use crate::{
    beacon::InMemoryBeaconCache,
    node::save_chain,
    p2p::Peer,
    state::State,
    update::{run_effect, update},
};

pub mod api;
pub mod beacon;
pub mod blockchain;
pub mod config;
pub mod node;
pub mod p2p;
pub mod state;
pub mod update;
pub mod util;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Whether to mine blocks
    #[arg(short, long)]
    mining: bool,

    /// The IP address to add to the peer list
    #[arg(short, long)]
    peer: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    logger::init_with_level(Level::Info).unwrap();

    let Some(mut state) = init_state() else {
        return;
    };
    debug!("address: {}", state.address.der);

    let (event_tx, mut event_rx) = mpsc::channel(256);
    let (state_tx, state_rx) = watch::channel(state.clone());
    init_p2p_and_api(state_rx, event_tx.clone()).await;
    let beacon_cache = Arc::new(InMemoryBeaconCache::new());

    if args.mining {
        let _ = event_tx.send(update::Event::MineBlock).await;
    }
    if let Some(address) = args.peer {
        match Ipv4Addr::from_str(&address) {
            Ok(ip) => {
                let _ = event_tx.send(update::Event::AddPeer(Peer::new(ip))).await;
            }
            Err(e) => error!("invalid ip address: {}", e),
        }
    }
    let mut previous_chain = state.chain.clone();

    while let Some(event) = event_rx.recv().await {
        let (new_state, effect) = update(event, state, beacon_cache.as_ref()).await;
        state = new_state.clone();
        if state.chain != previous_chain {
            let _ = save_chain(&state.chain).inspect_err(|e| error!("failed to save chain: {}", e));
            previous_chain = state.chain.clone();
        }
        let _ = state_tx.send(state.clone());
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let events = run_effect(new_state, effect).await;
            for event in events {
                let _ = event_tx_clone.send(event).await;
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
    let Ok(state) = state::State::new(sk, chain) else {
        error!("failed to initialize state");
        return None;
    };
    Some(state)
}

async fn init_p2p_and_api(
    state_rx: watch::Receiver<State>,
    event_tx: mpsc::Sender<update::Event>,
) -> () {
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        api::init_api(event_tx_clone, state_rx).await;
    });
    tokio::spawn(async move {
        p2p::init_p2p(event_tx).await;
    });
}
