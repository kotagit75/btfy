#[macro_use]
extern crate log;
extern crate simple_logger as logger;

use anyhow::{Ok, Result};
use clap::Parser;
use log::Level;
use std::sync::Arc;

use crate::{
    chain_repository::FileChainRepository, config::Config, key_repository::FileKeyRepository,
    node::Node,
};
use btfy_beacon::{cache::InMemoryBeaconCache, provider::command::CommandBeaconProvider};
use tokio::sync::Mutex;

pub mod api;
pub mod chain_repository;
pub mod config;
pub mod effect;
pub mod event;
pub mod key_repository;
pub mod node;
pub mod p2p;
pub mod state;

#[tokio::main]
async fn main() -> Result<()> {
    logger::init_with_level(Level::Info).unwrap();

    let config = Args::parse().to_config();

    let node = Node::new(
        config.clone(),
        Box::new(FileChainRepository::new("chain")),
        Box::new(FileKeyRepository::new("key.der")),
        Arc::new(InMemoryBeaconCache::new()),
        Arc::new(Mutex::new(CommandBeaconProvider::spawn(
            &config.beacon_cmd,
        )?)),
    )
    .await?;
    node.run().await;
    Ok(())
}

pub const VDF_DIFFICULTY: u64 = 5295676;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Whether to mine blocks
    #[arg(short, long)]
    pub mining: bool,

    /// The address to add to the peer list
    #[arg(short, long)]
    pub peer: Vec<String>,

    /// The port to listen on for the API
    #[arg(short, long, default_value = "8080")]
    pub api_port: u16,

    /// The port to listen on for the P2P network
    #[arg(long, default_value = "62697")]
    pub p2p_port: u16,

    /// The timeout for API requests in seconds
    #[arg(short, long, default_value = "5")]
    pub beacon_timeout: u64,

    /// Beacon provider command to run over stdio
    #[arg(long = "beacon-cmd", num_args = 1.., value_name = "CMD")]
    pub beacon_cmd: Vec<String>,

    /// For testing only: vdf difficulty
    #[arg(long)]
    pub vdf_difficulty: Option<u64>,
}

impl Args {
    pub fn to_config(&self) -> Config {
        Config {
            mining: self.mining,
            peer: self.peer.clone(),
            api_port: self.api_port,
            p2p_port: self.p2p_port,
            beacon_timeout: self.beacon_timeout,
            beacon_cmd: self.beacon_cmd.clone(),
            vdf_difficulty: self.vdf_difficulty.unwrap_or(VDF_DIFFICULTY),
        }
    }
}
