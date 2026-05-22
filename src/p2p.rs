use std::{
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
};

use axum::{Router, extract, response, routing::post};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    blockchain::{block::Block, transaction::Transaction},
    config::CONFIG,
    update::Event,
};

pub async fn init_p2p(event_tx: mpsc::Sender<Event>) {
    let app = Router::new()
        .route("/", post(handle_post_message))
        .with_state(event_tx);
    let addr = SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        CONFIG.internal_config.p2p_port,
    );
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("P2P server is running on http://{}/", addr);
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum P2PMessage {
    QueryLatest,
    QueryAll,
    QueryTransactions,
    QueryPeers,
    ResponseBlockChain(Vec<Block>),
    ResponseTransactions(Vec<Transaction>),
    ResponsePeers(Vec<Peer>),
}

async fn handle_post_message(
    extract::State(event_tx): extract::State<mpsc::Sender<Event>>,
    extract::ConnectInfo(peer_addr): extract::ConnectInfo<SocketAddr>,
    extract::Json(message): extract::Json<P2PMessage>,
) -> response::Json<bool> {
    response::Json(
        event_tx
            .send(Event::P2PMessage(
                match Ipv4Addr::from_str(&peer_addr.ip().to_string()) {
                    Ok(ip) => Some(Peer::new(ip)),
                    Err(_) => None,
                },
                message,
            ))
            .await
            .is_ok(),
    )
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Peer {
    pub ip: Ipv4Addr,
}
impl Peer {
    pub fn new(ip: Ipv4Addr) -> Self {
        Self { ip }
    }
    pub fn get_url(&self) -> String {
        format!(
            "http://{}/",
            SocketAddr::new(
                std::net::IpAddr::V4(self.ip),
                CONFIG.internal_config.p2p_port
            )
        )
    }
    pub async fn write(&self, message: &P2PMessage) {
        let result = reqwest::Client::new()
            .post(&self.get_url())
            .json(message)
            .send()
            .await;
        if result.is_err() {
            error!(
                "failed to send message to peer({}): {:?}",
                self.ip,
                result.err()
            );
        }
    }
}

pub async fn broadcast(peers: &[Peer], message: &P2PMessage) {
    for peer in peers {
        peer.write(message).await;
    }
}
