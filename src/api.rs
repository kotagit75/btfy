use std::{
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
};

use axum::{
    Router,
    extract::{self, Path},
    http::HeaderValue,
    response,
    routing::{get, post},
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    blockchain::{address::Address, chain::Chain},
    config::CONFIG,
    p2p::Peer,
    state::State,
    update::Event,
    util::key::PK,
};

pub async fn init_api(event_tx: mpsc::Sender<Event>, state_rx: watch::Receiver<State>) {
    let allowed_origin = format!("http://localhost:{}", CONFIG.cors_allow_port);
    let cors = CorsLayer::new()
        .allow_origin([allowed_origin.parse::<HeaderValue>().unwrap()])
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/address", get(handle_query_address))
        .route("/chain", get(handle_query_chain))
        .route("/balance", get(handle_query_balance))
        .route("/balance/{address}", get(handle_query_balance_with_address))
        .route("/tx", post(handle_command_transaction))
        .route("/peer", post(handle_command_peer))
        .with_state((event_tx, state_rx))
        .layer(cors);
    let addr = SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        CONFIG.api_port,
    );
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("API server is running on http://{}/", addr);
    info!("API server allows CORS for {}", allowed_origin);
    axum::serve(listener, app).await.unwrap();
}

async fn handle_query_address(
    extract::State((_, state_rx)): extract::State<(mpsc::Sender<Event>, watch::Receiver<State>)>,
) -> String {
    state_rx.borrow().clone().address.der
}

async fn handle_query_chain(
    extract::State((_, state_rx)): extract::State<(mpsc::Sender<Event>, watch::Receiver<State>)>,
) -> response::Json<Chain> {
    response::Json(state_rx.borrow().clone().chain)
}

async fn handle_query_balance(
    extract::State((_, state_rx)): extract::State<(mpsc::Sender<Event>, watch::Receiver<State>)>,
) -> response::Json<u64> {
    let state = state_rx.borrow().clone();
    response::Json(state.chain.get_balance(&state.address))
}

async fn handle_query_balance_with_address(
    extract::State((_, state_rx)): extract::State<(mpsc::Sender<Event>, watch::Receiver<State>)>,
    Path(address): Path<String>,
) -> response::Json<u64> {
    let state = state_rx.borrow().clone();
    response::Json(state.chain.get_balance(&Address { der: address }))
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct TransactionPayload {
    recipient: String,
    send_amount: u64,
    fee: u64,
}
async fn handle_command_transaction(
    extract::State((event_tx, _)): extract::State<(mpsc::Sender<Event>, watch::Receiver<State>)>,
    extract::Json(payload): extract::Json<TransactionPayload>,
) -> response::Json<bool> {
    response::Json(
        event_tx
            .send(Event::AddTransaction(
                PK {
                    der: payload.recipient,
                },
                payload.send_amount,
                payload.fee,
            ))
            .await
            .is_ok(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct PeerPayload {
    ip: String,
}
async fn handle_command_peer(
    extract::State((event_tx, _)): extract::State<(mpsc::Sender<Event>, watch::Receiver<State>)>,
    extract::Json(payload): extract::Json<PeerPayload>,
) -> response::Json<bool> {
    response::Json(match Ipv4Addr::from_str(&payload.ip) {
        Ok(ip) => event_tx.send(Event::AddPeer(Peer::new(ip))).await.is_ok(),
        Err(_) => false,
    })
}
