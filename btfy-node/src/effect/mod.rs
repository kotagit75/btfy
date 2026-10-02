use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, time};

use crate::{
    config::Config,
    event::Event,
    p2p::{P2PMessage, broadcast},
    state::State,
};
use btfy_beacon::{fetch_beacon, provider::BeaconProvider};
use btfy_core::{block::solve_block_vdf, transaction::Transaction};

pub mod runner;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    None,
    MineBlock(Vec<Transaction>),
    Broadcast(P2PMessage),
}

pub fn when_changed(effect: Effect, changed: bool) -> Effect {
    if changed { effect } else { Effect::None }
}

impl Effect {
    pub async fn run<T: BeaconProvider>(
        self,
        state: State,
        config: Config,
        beacon_provider: &Mutex<T>,
    ) -> Vec<Event> {
        match self {
            Effect::None => Vec::new(),
            Effect::MineBlock(transactions) => {
                info!("generating next block");
                let next_timestamp = Utc::now().timestamp();
                let Some(beacon) = ({
                    let mut beacon_provider = beacon_provider.lock().await;
                    fetch_beacon(
                        &mut *beacon_provider,
                        &state.chain.get_latest_block().hash,
                        next_timestamp,
                    )
                    .await
                }) else {
                    error!("failed to fetch beacon");
                    return vec![Event::MineBlock];
                };

                let now = time::Instant::now();
                let block_data = state.chain.generate_next_block_data(
                    &state.address,
                    beacon,
                    transactions,
                    next_timestamp,
                );
                let block_data_clone = block_data.clone();
                debug!("calculating vdf solution");
                let vdf_solution = tokio::task::spawn_blocking(move || {
                    solve_block_vdf(&block_data_clone, config.vdf_difficulty)
                })
                .await
                .unwrap()
                .unwrap();
                debug!("calculated vdf solution");

                let block =
                    state
                        .chain
                        .generate_next_block(&state.secret_key, vdf_solution, block_data);
                info!("generated next block: {}ms", now.elapsed().as_millis());
                vec![Event::CompletedMineBlock(block), Event::MineBlock]
            }
            Effect::Broadcast(message) => {
                vec![Event::RemovePeers(
                    broadcast(&state.peers, &message, config.p2p_port).await,
                )]
            }
        }
    }
}
