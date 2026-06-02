use std::{time, vec};

use crate::{
    beacon::{BeaconCache, get_beacon, prefetch_beacon},
    blockchain::{
        address::{Address, is_valid_address},
        block::{Block, MAX_TRANSACTIONS_PER_BLOCK},
        chain::{CHECKPOINT_DEPTH, Chain},
        transaction::Transaction,
    },
    p2p::{P2PMessage, Peer, broadcast},
    state::State,
};
use chrono::Utc;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Event {
    AddPeer(Peer),
    RemovePeers(Vec<Peer>),
    AddTransaction(Address, u64, u64),
    MineBlock,
    CompletedMineBlock(Block),
    P2PMessage(Option<Peer>, P2PMessage),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    None,
    MineBlock(Vec<Transaction>),
    Broadcast(P2PMessage),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdateResult {
    pub changed: bool,
    pub effect: Effect,
}

#[derive(Debug)]
pub enum Command {
    Event(Event),
    ApiRequest(Event, oneshot::Sender<UpdateResult>),
}

async fn prefetch_chain_beacons(cache: &dyn BeaconCache, blocks: &[Block]) {
    if blocks.len() < 2 {
        return;
    }
    let start = blocks.len().saturating_sub(CHECKPOINT_DEPTH + 1);
    let tasks = blocks[start..]
        .windows(2)
        .map(|window| prefetch_beacon(cache, &window[0].hash, window[1].timestamp));
    let _ = join_all(tasks).await;
}

pub async fn update(event: Event, state: State, beacon_cache: &dyn BeaconCache) -> (State, Effect) {
    match event {
        Event::AddPeer(peer) => {
            info!("added peer: {}", peer.ip);
            let (state, changed) = state.add_peer(&peer);
            return (
                state,
                if changed {
                    Effect::Broadcast(P2PMessage::QueryPeers)
                } else {
                    Effect::None
                },
            );
        }
        Event::RemovePeers(peers) => {
            info!(
                "remove peers: {:?}",
                peers.iter().map(|peer| peer.ip.to_string())
            );
            return (state.remove_peers(&peers).0, Effect::None);
        }
        Event::AddTransaction(recipient, send_amount, fee) => {
            if !is_valid_address(&recipient) {
                info!("invalid recipient address: {}", recipient.der);
                return (state, Effect::None);
            }
            if let Ok(Some(transaction)) = state.chain.generate_transaction(
                &state.address,
                &recipient,
                send_amount,
                &state.secret_key,
                &state.transactions,
                fee,
            ) {
                let (state, changed) = state.add_transaction(&transaction);
                if changed {
                    info!("added transaction: {:?}", transaction);
                }
                return (state, {
                    if changed {
                        Effect::Broadcast(P2PMessage::ResponseTransactions(vec![
                            transaction.clone(),
                        ]))
                    } else {
                        Effect::None
                    }
                });
            }
        }
        Event::MineBlock => {
            let mut sorted_transactions: Vec<_> = state.transactions.clone();
            sorted_transactions.sort_by_key(|tx| tx.fee);
            sorted_transactions.reverse();
            let (transactions_to_mine, remaining_transactions) = sorted_transactions.split_at(
                std::cmp::min(MAX_TRANSACTIONS_PER_BLOCK, sorted_transactions.len()),
            );

            println!("transactions to mine: {}", transactions_to_mine.len());

            return (
                State {
                    transactions: remaining_transactions.to_vec(),
                    ..state
                },
                Effect::MineBlock(transactions_to_mine.to_vec()),
            );
        }
        Event::CompletedMineBlock(new_block) => {
            let _ = prefetch_beacon(
                beacon_cache,
                &state.chain.get_latest_block().hash,
                new_block.timestamp,
            )
            .await;
            let (chain, changed) =
                state
                    .chain
                    .add_block(new_block.clone(), true, true, beacon_cache);
            let new_state = State { chain, ..state };

            if changed {
                info!("completed to add next block");
            } else {
                error!("failed to add next block");
            }

            return (new_state, {
                if changed {
                    Effect::Broadcast(P2PMessage::ResponseBlockChain(vec![new_block]))
                } else {
                    Effect::None
                }
            });
        }
        Event::P2PMessage(_, P2PMessage::QueryAll) => {
            let chain = state.chain.blocks.clone();
            return (
                state,
                Effect::Broadcast(P2PMessage::ResponseBlockChain(chain)),
            );
        }
        Event::P2PMessage(_, P2PMessage::QueryLatest) => {
            let blocks = vec![state.chain.get_latest_block()];
            return (
                state,
                Effect::Broadcast(P2PMessage::ResponseBlockChain(blocks)),
            );
        }
        Event::P2PMessage(_, P2PMessage::ResponseBlockChain(blocks)) => {
            let Some(received_latest_block) = blocks.last() else {
                return (state, Effect::None);
            };
            let held_latest_block = state.chain.get_latest_block();
            if received_latest_block.index > held_latest_block.index {
                if received_latest_block.previous_hash == held_latest_block.hash {
                    let _ = prefetch_beacon(
                        beacon_cache,
                        &held_latest_block.hash,
                        received_latest_block.timestamp,
                    )
                    .await;
                    let (new_chain, changed) = state.chain.add_block(
                        received_latest_block.clone(),
                        false,
                        true,
                        beacon_cache,
                    );
                    if changed {
                        info!("added block: {:?}", received_latest_block);
                    }
                    return (
                        State {
                            chain: new_chain,
                            ..state
                        },
                        {
                            if changed {
                                Effect::Broadcast(P2PMessage::ResponseBlockChain(vec![
                                    received_latest_block.clone(),
                                ]))
                            } else {
                                Effect::None
                            }
                        },
                    );
                } else if blocks.len() == 1 {
                    return (state, Effect::Broadcast(P2PMessage::QueryAll));
                } else {
                    prefetch_chain_beacons(beacon_cache, &blocks).await;
                    info!("replacing chain with {} blocks", blocks.len());
                    return (
                        State {
                            chain: state.chain.replace(Chain { blocks }, beacon_cache),
                            ..state
                        },
                        Effect::None,
                    );
                }
            }
        }
        Event::P2PMessage(_, P2PMessage::QueryTransactions) => {
            return (
                state.clone(),
                Effect::Broadcast(P2PMessage::ResponseTransactions(state.transactions.clone())),
            );
        }
        Event::P2PMessage(_, P2PMessage::ResponseTransactions(transactions)) => {
            let (state, changed) =
                transactions
                    .iter()
                    .fold((state, false), |(state, changed), transaction| {
                        let (state, changed_) = state.add_transaction(transaction);
                        (state, changed || changed_)
                    });
            return (state.clone(), {
                if changed {
                    Effect::Broadcast(P2PMessage::ResponseTransactions(state.transactions.clone()))
                } else {
                    Effect::None
                }
            });
        }
        Event::P2PMessage(peer_option, P2PMessage::QueryPeers) => {
            return (
                match peer_option {
                    Some(peer) => state.add_peer(&peer).0,
                    None => state.clone(),
                },
                Effect::Broadcast(P2PMessage::ResponsePeers(state.peers.clone())),
            );
        }
        Event::P2PMessage(_, P2PMessage::ResponsePeers(peers)) => {
            let (state, changed) = state.add_peers(&peers);
            return (state.clone(), {
                if changed {
                    Effect::Broadcast(P2PMessage::ResponsePeers(state.peers.clone()))
                } else {
                    Effect::None
                }
            });
        }
    }
    (state, Effect::None)
}

pub async fn run_effect(state: State, effect: Effect) -> Vec<Event> {
    match effect {
        Effect::None => {}
        Effect::MineBlock(transactions) => {
            info!("start generate next block");
            let next_timestamp = Utc::now().timestamp_millis();
            let Some(beacon) =
                get_beacon(&state.chain.get_latest_block().hash, next_timestamp).await
            else {
                error!("failed to get beacon");
                return vec![Event::MineBlock];
            };
            let now = time::Instant::now();
            let Ok(block) = state.chain.generate_next_block(
                &state.secret_key,
                &state.address,
                beacon,
                transactions,
                next_timestamp,
            ) else {
                error!("failed to generate next block");
                return vec![Event::MineBlock];
            };
            info!(
                "completed generate next block: {}ms",
                now.elapsed().as_millis()
            );
            return vec![Event::CompletedMineBlock(block), Event::MineBlock];
        }
        Effect::Broadcast(message) => {
            return vec![Event::RemovePeers(broadcast(&state.peers, &message).await)];
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        beacon::{Beacon, InMemoryBeaconCache},
        blockchain::{
            block::{Block, genesis_block},
            chain::Chain,
            transaction::coinbase_transaction,
        },
        state::State,
        util::key::{SK, generate_pk_and_sk},
    };
    use std::net::Ipv4Addr;

    fn keypair() -> (Address, SK) {
        let (pk, sk) = generate_pk_and_sk(512).unwrap();
        (pk, sk)
    }

    fn dummy_block_with_coinbase(prev: &Block, miner: &Address) -> Block {
        Block {
            index: prev.index + 1,
            timestamp: prev.timestamp + 1,
            transactions: vec![coinbase_transaction(miner, prev.index + 1)],
            beacon: Beacon { values: Vec::new() },
            vdf_solution: vec![],
            previous_hash: prev.hash,
            issuer: miner.clone(),
            signature: vec![],
            hash: [prev.index as u8 + 1; 32],
        }
    }

    fn funded_state() -> State {
        let (_, sk) = keypair();
        let mut state = State::new(sk, Chain::new()).unwrap();
        let g = genesis_block();
        let b1 = dummy_block_with_coinbase(&g, &state.address);
        state.chain = Chain {
            blocks: vec![g, b1],
        };
        state
    }

    fn build_tx(state: &State, recipient: &Address, amount: u64, fee: u64) -> Transaction {
        state
            .chain
            .generate_transaction(
                &state.address,
                recipient,
                amount,
                &state.secret_key,
                &state.transactions,
                fee,
            )
            .unwrap()
            .unwrap()
    }

    async fn run_update(event: Event, state: State) -> (State, Effect) {
        let cache = InMemoryBeaconCache::new();
        update(event, state, &cache).await
    }

    #[tokio::test]
    async fn add_peer_broadcasts_query_peers_on_change() {
        let state = funded_state();
        let peer = Peer::new(Ipv4Addr::new(127, 0, 0, 1));

        let (next, effect) = run_update(Event::AddPeer(peer.clone()), state).await;

        assert!(next.peers.contains(&peer));
        assert_eq!(effect, Effect::Broadcast(P2PMessage::QueryPeers));
    }

    #[tokio::test]
    async fn add_peer_duplicate_does_not_broadcast() {
        let mut state = funded_state();
        let peer = Peer::new(Ipv4Addr::new(127, 0, 0, 1));
        state = state.add_peer(&peer).0;

        let (next, effect) = run_update(Event::AddPeer(peer.clone()), state.clone()).await;

        assert_eq!(next, state);
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn remove_peers_removes_and_broadcasts_query_peers() {
        let mut state = funded_state();
        let p1 = Peer::new(Ipv4Addr::new(10, 0, 0, 1));
        let p2 = Peer::new(Ipv4Addr::new(10, 0, 0, 2));
        state = state.add_peer(&p1).0;
        state = state.add_peer(&p2).0;

        let (next, effect) = run_update(Event::RemovePeers(vec![p1.clone()]), state).await;

        assert!(!next.peers.contains(&p1));
        assert!(next.peers.contains(&p2));
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn add_transaction_rejects_invalid_recipient() {
        let state = funded_state();
        let invalid = Address {
            der: "this-is-not-hex".to_string(),
        };

        let (next, effect) = run_update(Event::AddTransaction(invalid, 10, 0), state.clone()).await;

        assert_eq!(effect, Effect::None);
        assert_eq!(next, state);
    }

    #[tokio::test]
    async fn add_transaction_accepts_and_broadcasts_when_valid() {
        let state = funded_state();
        let (recipient, _) = keypair();

        let (next, effect) = run_update(Event::AddTransaction(recipient, 10, 0), state).await;

        assert_eq!(next.transactions.len(), 1);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponseTransactions(next.transactions.clone()))
        );
    }

    #[tokio::test]
    async fn add_transaction_with_fee_is_rejected_when_not_enough_for_fee() {
        let state = funded_state();
        let (recipient, _) = keypair();

        let (next, effect) =
            run_update(Event::AddTransaction(recipient, 50, 1), state.clone()).await;

        assert_eq!(next, state);
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn add_transaction_with_fee_is_broadcast_when_sufficient() {
        let state = funded_state();
        let (recipient, _) = keypair();

        let (next, effect) = run_update(Event::AddTransaction(recipient, 48, 2), state).await;

        assert_eq!(next.transactions.len(), 1);
        assert_eq!(next.transactions[0].fee, 2);
        match effect {
            Effect::Broadcast(P2PMessage::ResponseTransactions(txs)) => {
                assert_eq!(txs.len(), 1);
                assert_eq!(txs[0].fee, 2);
            }
            _ => panic!("expected ResponseTransactions broadcast"),
        }
    }

    #[tokio::test]
    async fn mine_block_clears_pending_and_returns_sorted_transactions() {
        let mut state = funded_state();
        let tx1 = Transaction {
            sender: state.address.clone(),
            out: Vec::new(),
            tx_in: Vec::new(),
            fee: 1,
            signature: Vec::new(),
        };
        let tx2 = Transaction {
            sender: state.address.clone(),
            out: Vec::new(),
            tx_in: Vec::new(),
            fee: 3,
            signature: Vec::new(),
        };
        state.transactions = vec![tx1, tx2];

        let (next, effect) = run_update(Event::MineBlock, state).await;

        assert!(next.transactions.is_empty());
        match effect {
            Effect::MineBlock(mined) => {
                assert_eq!(mined.len(), 2);
                assert_eq!(mined[0].fee, 3);
                assert_eq!(mined[1].fee, 1);
            }
            _ => panic!("expected MineBlock effect"),
        }
    }

    #[tokio::test]
    async fn mine_block_limits_transactions_to_max_and_prioritizes_fees() {
        let mut state = funded_state();
        let total = MAX_TRANSACTIONS_PER_BLOCK + 5;
        let txs: Vec<Transaction> = (0..total)
            .map(|i| Transaction {
                sender: state.address.clone(),
                out: Vec::new(),
                tx_in: Vec::new(),
                fee: i as u64,
                signature: Vec::new(),
            })
            .collect();
        state.transactions = txs;

        let (_, effect) = run_update(Event::MineBlock, state).await;

        match effect {
            Effect::MineBlock(mined) => {
                assert_eq!(mined.len(), MAX_TRANSACTIONS_PER_BLOCK);

                let expected_highest = (total - 1) as u64;
                let expected_lowest = (total - MAX_TRANSACTIONS_PER_BLOCK) as u64;

                assert_eq!(mined.first().unwrap().fee, expected_highest);
                assert_eq!(mined.last().unwrap().fee, expected_lowest);
                assert!(mined.windows(2).all(|w| w[0].fee >= w[1].fee));
            }
            _ => panic!("expected MineBlock effect"),
        }
    }

    #[tokio::test]
    async fn query_transactions_returns_current_pool() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let tx = build_tx(&state, &recipient, 10, 0);
        state.transactions.push(tx.clone());
        let expected = state.transactions.clone();

        let (next, effect) = run_update(
            Event::P2PMessage(None, P2PMessage::QueryTransactions),
            state.clone(),
        )
        .await;

        assert_eq!(next, state);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponseTransactions(expected))
        );
    }

    #[tokio::test]
    async fn response_transactions_adds_new_and_rebroadcasts() {
        let state = funded_state();
        let (recipient, _) = keypair();
        let tx = build_tx(&state, &recipient, 10, 0);

        let (next, effect) = run_update(
            Event::P2PMessage(None, P2PMessage::ResponseTransactions(vec![tx.clone()])),
            state,
        )
        .await;

        assert_eq!(next.transactions, vec![tx.clone()]);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponseTransactions(vec![tx]))
        );
    }

    #[tokio::test]
    async fn response_transactions_duplicate_is_ignored() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let tx = build_tx(&state, &recipient, 10, 0);
        state.transactions.push(tx.clone());

        let (next, effect) = run_update(
            Event::P2PMessage(None, P2PMessage::ResponseTransactions(vec![tx])),
            state.clone(),
        )
        .await;

        assert_eq!(next, state);
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn query_peers_adds_sender_and_responds_with_known_peers() {
        let mut state = funded_state();
        let existing = Peer::new(Ipv4Addr::new(10, 0, 0, 1));
        state = state.add_peer(&existing).0;
        let sender = Peer::new(Ipv4Addr::new(10, 0, 0, 2));

        let (next, effect) = run_update(
            Event::P2PMessage(Some(sender.clone()), P2PMessage::QueryPeers),
            state.clone(),
        )
        .await;

        assert!(next.peers.contains(&existing));
        assert!(next.peers.contains(&sender));
        assert_eq!(next.peers.len(), 2);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponsePeers(vec![existing]))
        );
    }

    #[tokio::test]
    async fn query_peers_without_sender_returns_current_list() {
        let mut state = funded_state();
        let existing = Peer::new(Ipv4Addr::new(10, 0, 0, 1));
        state = state.add_peer(&existing).0;

        let (next, effect) = run_update(
            Event::P2PMessage(None, P2PMessage::QueryPeers),
            state.clone(),
        )
        .await;

        assert_eq!(next, state);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponsePeers(vec![existing]))
        );
    }

    #[tokio::test]
    async fn response_peers_merges_and_rebroadcasts() {
        let mut state = funded_state();
        let existing = Peer::new(Ipv4Addr::new(10, 0, 0, 1));
        let new_peer = Peer::new(Ipv4Addr::new(10, 0, 0, 2));
        state = state.add_peer(&existing).0;

        let (next, effect) = run_update(
            Event::P2PMessage(None, P2PMessage::ResponsePeers(vec![new_peer.clone()])),
            state,
        )
        .await;

        assert!(next.peers.contains(&existing));
        assert!(next.peers.contains(&new_peer));
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponsePeers(next.peers.clone()))
        );
    }

    #[tokio::test]
    async fn response_peers_duplicate_is_ignored() {
        let mut state = funded_state();
        let existing = Peer::new(Ipv4Addr::new(10, 0, 0, 1));
        state = state.add_peer(&existing).0;

        let (next, effect) = run_update(
            Event::P2PMessage(None, P2PMessage::ResponsePeers(vec![existing.clone()])),
            state.clone(),
        )
        .await;

        assert_eq!(next, state);
        assert_eq!(effect, Effect::None);
    }
}
