use std::sync::Arc;

use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};

use btfy_beacon::provider::BeaconProvider;

use crate::{config::Config, effect::Effect, event::command::Command, state::State};

pub struct EffectRunner<T: BeaconProvider + 'static> {
    event_tx: mpsc::Sender<Command>,
    config: Config,
    beacon_provider: Arc<Mutex<T>>,
    mining_tasks: Vec<JoinHandle<()>>,
}

impl<T: BeaconProvider + 'static> EffectRunner<T> {
    pub fn new(
        event_tx: mpsc::Sender<Command>,
        config: Config,
        beacon_provider: Arc<Mutex<T>>,
    ) -> Self {
        Self {
            event_tx,
            config,
            beacon_provider,
            mining_tasks: Vec::new(),
        }
    }

    pub fn spawn_effect(&mut self, effect: Effect, state: State) {
        let event_tx = self.event_tx.clone();
        let config = self.config.clone();
        let beacon_provider = Arc::clone(&self.beacon_provider);
        let effect_clone = effect.clone();

        let task = tokio::spawn(async move {
            let events = effect_clone.run(state, config, &beacon_provider).await;

            for event in events {
                if event_tx.send(Command::Event(event)).await.is_err() {
                    break;
                }
            }
        });

        if let Effect::MineBlock(_) = effect {
            self.mining_tasks.push(task);
        }
    }

    pub fn cancel_mining(&mut self) {
        for task in &mut self.mining_tasks {
            task.abort();
        }
        self.mining_tasks.clear();
    }
}
