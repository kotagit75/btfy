use std::vec;

use crate::{
    beacon::get_beacon,
    blockchain::{
        address::{Address, is_valid_address},
        block::Block,
        chain::Chain,
        transaction::{Transaction, coinbase_transaction},
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
            let coinbase = coinbase_transaction(&state.address);
            let blocks_for_mine: Vec<Transaction> = [coinbase]
                .iter()
                .chain(&state.transactions)
                .cloned()
                .collect();
            return (
                State {
                    transactions: Vec::new(),
                    ..state
                },
                Effect::MineBlock(blocks_for_mine),
            );
        }
        Event::CompletedMineBlock(new_block) => {
            info!("completed mining block");
            let (chain, changed) = state.chain.add_block(new_block.clone(), true, true);
            println!("changed: {}", changed);
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
            return (
                state.clone(),
                Effect::BroadcastResponseBlocks(state.chain.blocks.clone()),
            );
        }
        Event::P2PMessage(P2PMessage::QueryLatest) => {
            return (
                state.clone(),
                Effect::BroadcastResponseBlocks(vec![state.chain.get_latest_block()]),
            );
        }
        Event::P2PMessage(P2PMessage::ResponseBlockChain(blocks)) => {
            let Some(received_lastest_block) = blocks.last() else {
                return (state, Effect::None);
            };
            let held_lastest_block = state.chain.get_latest_block();
            if received_lastest_block.index > held_lastest_block.index {
                if received_lastest_block.previous_hash == held_lastest_block.hash {
                    let (new_chain, changed) =
                        state
                            .chain
                            .add_block(received_lastest_block.clone(), false, true);
                    return (
                        State {
                            chain: new_chain,
                            ..state
                        },
                        {
                            if changed {
                                Effect::BroadcastResponseBlocks(vec![
                                    received_lastest_block.clone(),
                                ])
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
