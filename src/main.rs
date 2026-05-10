#[macro_use]
extern crate log;
extern crate simple_logger as logger;

extern crate regex;

use log::Level;
use tokio::sync::{mpsc, watch};

use crate::update::{run_effect, update};

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

    info!("loading node key");
    let Ok(sk) = node::load_key() else {
        error!("failed to load node key");
        return;
    };
    info!("initializing state");
    let Ok(mut state) = state::State::new(sk) else {
        error!("failed to initialize state");
        return;
    };
    debug!("address: {}", state.address.der);

    let (event_tx, mut event_rx) = mpsc::channel(256);
    let (state_tx, state_rx) = watch::channel(state.clone());
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        api::init_api(event_tx_clone, state_rx).await;
    });
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        p2p::init_p2p(event_tx_clone).await;
    });

    let _ = event_tx.send(update::Event::MineBlock).await;

    while let Some((new_state, effect)) = event_rx.recv().await.map(|event| update(event, state)) {
        state = new_state.clone();
        let _ = state_tx.send(state.clone());
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            run_effect(new_state, event_tx_clone, effect).await;
        });
    }
}
