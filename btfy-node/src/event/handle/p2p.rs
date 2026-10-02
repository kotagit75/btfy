use crate::{
    effect::{Effect, when_changed},
    event::beacon::prefetch_chain_beacons,
    p2p::{P2PMessage, Peer},
    state::State,
};
use btfy_beacon::{cache::BeaconCache, prefetch_beacon, provider::BeaconProvider};
use btfy_core::{block::Block, chain::Chain, transaction::Transaction};

pub async fn handle_p2p_message<T: BeaconProvider>(
    state: &mut State,
    beacon_cache: &dyn BeaconCache,
    peer_option: Option<Peer>,
    message: P2PMessage,
    vdf_difficulty: u64,
    beacon_provider: &mut T,
) -> Effect {
    match message {
        P2PMessage::QueryAll => handle_query_all(state),
        P2PMessage::QueryLatest => handle_query_latest(state),
        P2PMessage::ResponseBlockChain(blocks) => {
            handle_response_block_chain(
                state,
                blocks,
                beacon_cache,
                vdf_difficulty,
                beacon_provider,
            )
            .await
        }
        P2PMessage::QueryTransactions => handle_query_transactions(state),
        P2PMessage::ResponseTransactions(transactions) => {
            handle_response_transactions(state, transactions)
        }
        P2PMessage::QueryPeers => handle_query_peers(state, peer_option),
        P2PMessage::ResponsePeers(peers) => handle_response_peers(state, peers),
    }
}

fn handle_query_all(state: &State) -> Effect {
    let chain = state.chain.blocks.clone();
    Effect::Broadcast(P2PMessage::ResponseBlockChain(chain))
}

fn handle_query_latest(state: &State) -> Effect {
    let blocks = vec![state.chain.get_latest_block()];
    Effect::Broadcast(P2PMessage::ResponseBlockChain(blocks))
}

async fn handle_response_block_chain<T: BeaconProvider>(
    state: &mut State,
    blocks: Vec<Block>,
    beacon_cache: &dyn BeaconCache,
    vdf_difficulty: u64,
    beacon_provider: &mut T,
) -> Effect {
    let Some(received_latest_block) = blocks.last() else {
        return Effect::None;
    };
    let held_latest_block = state.chain.get_latest_block();
    if received_latest_block.index > held_latest_block.index {
        if received_latest_block.previous_hash == held_latest_block.hash {
            let _ = prefetch_beacon(
                beacon_provider,
                beacon_cache,
                &held_latest_block.hash,
                received_latest_block.timestamp,
            )
            .await;
            let changed = state.chain.add_block(
                received_latest_block.clone(),
                Some(beacon_cache),
                vdf_difficulty,
            );
            if changed {
                info!("added block to chain");
            }
            return when_changed(
                Effect::Broadcast(P2PMessage::ResponseBlockChain(vec![
                    received_latest_block.clone(),
                ])),
                changed,
            );
        } else if blocks.len() == 1 {
            return Effect::Broadcast(P2PMessage::QueryAll);
        } else {
            prefetch_chain_beacons(beacon_provider, beacon_cache, &blocks).await;
            info!("replacing chain with {} blocks", blocks.len());
            state
                .chain
                .replace(Chain { blocks }, beacon_cache, vdf_difficulty);
            return Effect::Broadcast(P2PMessage::QueryAll);
        }
    }
    Effect::None
}

fn handle_query_transactions(state: &State) -> Effect {
    Effect::Broadcast(P2PMessage::ResponseTransactions(state.transactions.clone()))
}

fn handle_response_transactions(state: &mut State, transactions: Vec<Transaction>) -> Effect {
    let mut changed = false;
    for transaction in transactions {
        changed = changed || state.add_transaction_to_pool(&transaction);
    }
    when_changed(
        Effect::Broadcast(P2PMessage::ResponseTransactions(state.transactions.clone())),
        changed,
    )
}

fn handle_query_peers(state: &mut State, peer_option: Option<Peer>) -> Effect {
    let known_peers = state.peers.clone();
    if let Some(peer) = peer_option {
        state.add_peers(&[peer]);
    }
    Effect::Broadcast(P2PMessage::ResponsePeers(known_peers))
}

fn handle_response_peers(state: &mut State, peers: Vec<Peer>) -> Effect {
    let changed = state.add_peers(&peers);
    when_changed(
        Effect::Broadcast(P2PMessage::ResponsePeers(state.peers.clone())),
        changed,
    )
}
