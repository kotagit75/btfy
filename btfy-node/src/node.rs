use std::io::Error;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc, watch};

use crate::{
    api,
    chain_repository::ChainRepository,
    config::Config,
    effect::Effect,
    event::{Event, command::Command},
    key_repository::KeyRepository,
    p2p::{self, Peer},
    state::State,
};
use btfy_beacon::{cache::BeaconCache, provider::BeaconProvider};

pub struct Node<T: BeaconProvider + 'static> {
    state: State,
    config: Config,
    chain_repo: Box<dyn ChainRepository>,
    event_tx: mpsc::Sender<Command>,
    event_rx: mpsc::Receiver<Command>,
    state_tx: watch::Sender<State>,
    beacon_cache: Arc<dyn BeaconCache>,
    beacon_provider: Arc<Mutex<T>>,
}

impl<T: BeaconProvider + 'static> Node<T> {
    pub async fn new(
        config: Config,
        chain_repo: Box<dyn ChainRepository>,
        key_repo: Box<dyn KeyRepository>,
        beacon_cache: Arc<dyn BeaconCache>,
        beacon_provider: Arc<Mutex<T>>,
    ) -> Result<Self, Error> {
        let state = init_state(chain_repo.as_ref(), key_repo.as_ref())?;
        let (event_tx, event_rx) = mpsc::channel(256);
        let (state_tx, state_rx) = watch::channel(state.clone());
        init_p2p_and_api(state_rx, event_tx.clone(), config.api_port, config.p2p_port).await;
        Ok(Self {
            state,
            config,
            chain_repo,
            beacon_cache,
            beacon_provider,
            event_tx,
            event_rx,
            state_tx,
        })
    }

    pub async fn run(mut self) {
        self.dispatch_initial_events().await;
        self.run_event_loop().await;
    }

    async fn dispatch_initial_events(&mut self) {
        if self.config.mining {
            let _ = self.event_tx.send(Command::Event(Event::MineBlock)).await;
        }
        for peer_address in self.config.peer.iter() {
            let _ = self
                .event_tx
                .send(Command::Event(Event::AddPeer(Peer::new(peer_address))))
                .await;
        }
    }

    async fn run_event_loop(&mut self) {
        while let Some(command) = self.event_rx.recv().await {
            let event = command.into_event();
            let result = event
                .process(
                    &mut self.state,
                    self.beacon_cache.as_ref(),
                    &mut *self.beacon_provider.lock().await,
                    &self.config,
                )
                .await;
            if result.chain_changed {
                let _ = self
                    .chain_repo
                    .save(&self.state.chain)
                    .inspect_err(|e| error!("failed to save chain: {}", e));
            }
            if result.changed {
                let _ = self.state_tx.send(self.state.clone());
            }
            if let Some(response_tx) = command.into_response_tx() {
                let _ = response_tx.send(result.clone());
            }
            self.spawn_effect(result.effect);
        }
    }

    fn spawn_effect(&self, effect: Effect) {
        let event_tx_clone = self.event_tx.clone();
        let state_clone = self.state.clone();
        let config_clone = self.config.clone();
        let beacon_provider_clone = Arc::clone(&self.beacon_provider);
        tokio::spawn(async move {
            let events = effect
                .run(state_clone, config_clone, &beacon_provider_clone)
                .await;
            for event in events {
                let _ = event_tx_clone.send(Command::Event(event)).await;
            }
        });
    }
}

fn init_state(
    chain_repo: &dyn ChainRepository,
    key_repo: &dyn KeyRepository,
) -> Result<State, Error> {
    debug!("loading node key");
    let sk = key_repo
        .load_or_init()
        .inspect_err(|e| error!("failed to load node key: {}", e))?;
    debug!("loading chain");
    let chain = chain_repo
        .load_or_init()
        .inspect_err(|e| error!("failed to load chain: {}", e))?;
    debug!("initializing state");
    Ok(State::new(sk, chain))
}

async fn init_p2p_and_api(
    state_rx: watch::Receiver<State>,
    event_tx: mpsc::Sender<Command>,
    api_port: u16,
    p2p_port: u16,
) -> () {
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        api::init_api(event_tx_clone, state_rx, api_port)
            .await
            .expect_err("failed to init api");
    });
    tokio::spawn(async move {
        p2p::init_p2p(event_tx, p2p_port)
            .await
            .expect_err("failed to init p2p");
    });
}
