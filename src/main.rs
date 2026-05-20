#[macro_use]
extern crate log;
extern crate simple_logger as logger;

extern crate regex;

use log::Level;
use tokio::sync::{mpsc, watch};

use crate::{
    node::save_chain,
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

    let _ = event_tx.send(update::Event::MineBlock).await;
    let mut previous_chain = state.chain.clone();

    while let Some((new_state, effect)) = event_rx.recv().await.map(|event| update(event, state)) {
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
