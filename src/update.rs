use std::vec;

use crate::{
    beacon::get_beacon,
    blockchain::{
        address::{Address, is_valid_address},
        block::Block,
        chain::Chain,
        transaction::Transaction,
    },
    p2p::{P2PMessage, Peer, broadcast},
    state::State,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Event {
    AddPeer(Peer),
    AddTransaction(Address, u64),
    MineBlock,
    CompletedMineBlock(Block),
    P2PMessage(P2PMessage),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    None,
    MineBlock(Vec<Transaction>),
    BroadcastQueryAll,
    BroadcastResponseBlocks(Vec<Block>),
    BroadcastResponseTransactions(Vec<Transaction>),
}

pub fn update(event: Event, state: State) -> (State, Effect) {
    match event {
        Event::AddPeer(peer) => {
            info!("added peer: {}", peer.ip);
            let new_peers: Vec<Peer> = state.peers.into_iter().chain([peer]).collect();
            return (
                State {
                    peers: new_peers,
                    ..state
                },
                Effect::None,
            );
        }
        Event::AddTransaction(recipient, amount) => {
            if !is_valid_address(&recipient) {
                info!("invalid recipient address: {}", recipient.der);
                return (state, Effect::None);
            }
            if let Ok(Some(transaction)) = state.chain.generate_transaction(
                &state.address,
                &recipient,
                amount,
                &state.secret_key,
                &state.transactions,
            ) {
                let (state, changed) = state.add_transaction(&transaction);
                if changed {
                    info!("added transaction: {:?}", transaction);
                }
                return (state, {
                    if changed {
                        Effect::BroadcastResponseTransactions(vec![transaction.clone()])
                    } else {
                        Effect::None
                    }
                });
            }
        }
        Event::MineBlock => {
            return (
                State {
                    transactions: Vec::new(),
                    ..state
                },
                Effect::MineBlock(state.transactions),
            );
        }
        Event::CompletedMineBlock(new_block) => {
            info!("completed mining block");
            let (chain, changed) = state.chain.add_block(new_block.clone(), true, true);
            let new_state = State { chain, ..state };
            return (new_state, {
                if changed {
                    Effect::BroadcastResponseBlocks(vec![new_block])
                } else {
                    Effect::None
                }
            });
        }
        Event::P2PMessage(P2PMessage::QueryAll) => {
            let chain = state.chain.blocks.clone();
            return (state, Effect::BroadcastResponseBlocks(chain));
        }
        Event::P2PMessage(P2PMessage::QueryLatest) => {
            let blocks = vec![state.chain.get_latest_block()];
            return (state, Effect::BroadcastResponseBlocks(blocks));
        }
        Event::P2PMessage(P2PMessage::ResponseBlockChain(blocks)) => {
            let Some(received_latest_block) = blocks.last() else {
                return (state, Effect::None);
            };
            let held_latest_block = state.chain.get_latest_block();
            if received_latest_block.index > held_latest_block.index {
                if received_latest_block.previous_hash == held_latest_block.hash {
                    let (new_chain, changed) =
                        state
                            .chain
                            .add_block(received_latest_block.clone(), false, true);
                    return (
                        State {
                            chain: new_chain,
                            ..state
                        },
                        {
                            if changed {
                                Effect::BroadcastResponseBlocks(vec![received_latest_block.clone()])
                            } else {
                                Effect::None
                            }
                        },
                    );
                } else if blocks.len() == 1 {
                    return (state, Effect::BroadcastQueryAll);
                } else {
                    return (
                        State {
                            chain: state.chain.replace(Chain { blocks }),
                            ..state
                        },
                        Effect::None,
                    );
                }
            }
        }
        Event::P2PMessage(P2PMessage::QueryTransactions) => {
            return (
                state.clone(),
                Effect::BroadcastResponseTransactions(state.transactions.clone()),
            );
        }
        Event::P2PMessage(P2PMessage::ResponseTransactions(transactions)) => {
            let (state, changed) =
                transactions
                    .iter()
                    .fold((state, false), |(state, changed), transaction| {
                        let (state, changed_) = state.add_transaction(transaction);
                        (state, changed || changed_)
                    });
            return (state.clone(), {
                if changed {
                    Effect::BroadcastResponseTransactions(state.transactions.clone())
                } else {
                    Effect::None
                }
            });
        }
    }
    (state, Effect::None)
}

pub async fn run_effect(state: State, event_tx: mpsc::Sender<Event>, effect: Effect) {
    match effect {
        Effect::None => {}
        Effect::MineBlock(transactions) => {
            info!("start mining block");
            let Some(beacon) = get_beacon(&state.chain.get_latest_block().hash) else {
                info!("stopping mining");
                return;
            };
            let Ok(block) = state.chain.generate_next_block(
                &state.secret_key,
                &state.address,
                beacon,
                transactions,
            ) else {
                return;
            };
            let _ = event_tx.send(Event::CompletedMineBlock(block)).await;
            let _ = event_tx.send(Event::MineBlock).await;
        }
        Effect::BroadcastResponseBlocks(blocks) => {
            broadcast(&state.peers, &P2PMessage::ResponseBlockChain(blocks)).await;
        }
        Effect::BroadcastQueryAll => {
            broadcast(&state.peers, &P2PMessage::QueryAll).await;
        }
        Effect::BroadcastResponseTransactions(transactions) => {
            broadcast(
                &state.peers,
                &P2PMessage::ResponseTransactions(transactions),
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        beacon::Beacon,
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

    #[test]
    fn add_peer_appends_peer() {
        let state = funded_state();
        let peer = Peer::new(Ipv4Addr::new(127, 0, 0, 1));

        let (next, effect) = update(Event::AddPeer(peer.clone()), state);

        assert_eq!(effect, Effect::None);
        assert!(next.peers.contains(&peer));
    }

    #[test]
    fn add_transaction_rejects_invalid_recipient() {
        let state = funded_state();
        let invalid = Address {
            der: "this-is-not-hex".to_string(),
        };

        let (next, effect) = update(Event::AddTransaction(invalid, 10), state.clone());

        assert_eq!(effect, Effect::None);
        assert_eq!(next, state);
    }

    #[test]
    fn add_transaction_accepts_and_broadcasts_when_valid() {
        let state = funded_state();
        let (recipient, _) = keypair();

        let (next, effect) = update(Event::AddTransaction(recipient, 10), state);

        assert_eq!(next.transactions.len(), 1);
        match effect {
            Effect::BroadcastResponseTransactions(txs) => assert_eq!(txs.len(), 1),
            _ => panic!("expected BroadcastResponseTransactions"),
        }
    }

    #[test]
    fn mine_block_clears_pending_and_creates_coinbase_first() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let tx = state
            .chain
            .generate_transaction(&state.address, &recipient, 10, &state.secret_key, &[])
            .unwrap()
            .unwrap();
        state.transactions.push(tx);

        let (next, effect) = update(Event::MineBlock, state);

        assert!(next.transactions.is_empty());
        match effect {
            Effect::MineBlock(txs) => {
                assert_eq!(txs.len(), 1);
            }
            _ => panic!("expected MineBlock effect"),
        }
    }

    #[test]
    fn query_transactions_returns_current_pool() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let tx = state
            .chain
            .generate_transaction(&state.address, &recipient, 10, &state.secret_key, &[])
            .unwrap()
            .unwrap();
        state.transactions.push(tx.clone());

        let (next, effect) = update(
            Event::P2PMessage(P2PMessage::QueryTransactions),
            state.clone(),
        );

        assert_eq!(next, state);
        assert_eq!(effect, Effect::BroadcastResponseTransactions(vec![tx]));
    }

    #[test]
    fn response_transactions_adds_new_and_rebroadcasts() {
        let state = funded_state();
        let (recipient, _) = keypair();
        let tx = state
            .chain
            .generate_transaction(&state.address, &recipient, 10, &state.secret_key, &[])
            .unwrap()
            .unwrap();

        let (next, effect) = update(
            Event::P2PMessage(P2PMessage::ResponseTransactions(vec![tx.clone()])),
            state,
        );

        assert_eq!(next.transactions.len(), 1);
        assert_eq!(effect, Effect::BroadcastResponseTransactions(vec![tx]));
    }

    #[test]
    fn response_transactions_duplicate_is_ignored() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let tx = state
            .chain
            .generate_transaction(&state.address, &recipient, 10, &state.secret_key, &[])
            .unwrap()
            .unwrap();
        state.transactions.push(tx.clone());

        let (next, effect) = update(
            Event::P2PMessage(P2PMessage::ResponseTransactions(vec![tx])),
            state.clone(),
        );

        assert_eq!(next, state);
        assert_eq!(effect, Effect::None);
    }
}
