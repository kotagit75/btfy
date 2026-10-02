use crate::{
    config::Config,
    effect::Effect,
    event::handle::{
        miner::{handle_completed_mine_block, handle_mine_block},
        p2p::handle_p2p_message,
        peer::{handle_add_peer, handle_remove_peers},
        transaction::handle_add_transaction,
    },
    p2p::{P2PMessage, Peer},
    state::State,
};
use btfy_beacon::{cache::BeaconCache, provider::BeaconProvider};
use btfy_core::{address::Address, block::Block};
use serde::{Deserialize, Serialize};

pub mod beacon;
pub mod command;
pub mod handle;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdateResult {
    pub changed: bool,
    pub chain_changed: bool,
    pub effect: Effect,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Event {
    AddPeer(Peer),
    RemovePeers(Vec<Peer>),
    AddTransaction(Address, u64, u64),
    MineBlock,
    CompletedMineBlock(Block),
    P2PMessage(Option<Peer>, P2PMessage),
}
impl Event {
    pub async fn process<T: BeaconProvider>(
        self,
        state: &mut State,
        beacon_cache: &dyn BeaconCache,
        beacon_provider: &mut T,
        config: &Config,
    ) -> UpdateResult {
        let previous_state = state.clone();
        let effect = match self {
            Event::AddPeer(peer) => handle_add_peer(state, peer),
            Event::RemovePeers(peers) => handle_remove_peers(state, peers),
            Event::AddTransaction(recipient, send_amount, fee) => {
                handle_add_transaction(state, &recipient, send_amount, fee)
            }
            Event::P2PMessage(peer_option, message) => {
                handle_p2p_message(
                    state,
                    beacon_cache,
                    peer_option,
                    message,
                    config.vdf_difficulty,
                    beacon_provider,
                )
                .await
            }
            Event::MineBlock => handle_mine_block(state),
            Event::CompletedMineBlock(new_block) => {
                handle_completed_mine_block(state, new_block, config.vdf_difficulty).await
            }
        };
        UpdateResult {
            changed: state != &previous_state,
            chain_changed: state.chain != previous_state.chain,
            effect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use btfy_beacon::{Beacon, cache::InMemoryBeaconCache, provider::dummy::DummyBeaconProvider};
    use btfy_core::{
        address::Address,
        block::{Block, MAX_TRANSACTIONS_PER_BLOCK, genesis_block},
        chain::Chain,
        coinbase::coinbase_transaction,
        transaction::Transaction,
    };
    use btfy_util::{
        key::{SK, generate_sk},
        signature::SignatureWrapper,
    };

    fn keypair() -> (Address, SK) {
        let sk = generate_sk(512);
        let pk = sk.to_pk();
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
            signature: SignatureWrapper::default(),
            hash: [prev.index as u8 + 1; 32],
        }
    }

    fn funded_state() -> State {
        let (_, sk) = keypair();
        let mut state = State::new(sk, Chain::new());
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
    }

    fn add_peers(mut state: State, new_peers: &[Peer]) -> (State, bool) {
        let changed = state.add_peers(new_peers);
        (state, changed)
    }

    async fn run_update(event: Event, state: &mut State) -> Effect {
        let cache = InMemoryBeaconCache::new();
        let mut provider = DummyBeaconProvider;
        event
            .process(state, &cache, &mut provider, &Config::default())
            .await
            .effect
    }

    #[tokio::test]
    async fn add_peer_broadcasts_query_peers_on_change() {
        let mut state = funded_state();
        let peer = Peer::new("127.0.0.1:8080");

        let effect = run_update(Event::AddPeer(peer.clone()), &mut state).await;

        assert!(state.peers.contains(&peer));
        assert_eq!(effect, Effect::Broadcast(P2PMessage::QueryPeers));
    }

    #[tokio::test]
    async fn add_peer_duplicate_does_not_broadcast() {
        let mut state = funded_state();
        let peer = Peer::new("127.0.0.1:8080");
        state = add_peers(state, std::slice::from_ref(&peer)).0;
        let previous_state = state.clone();

        let effect = run_update(Event::AddPeer(peer.clone()), &mut state).await;

        assert_eq!(state, previous_state);
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn remove_peers_removes_and_broadcasts_query_peers() {
        let mut state = funded_state();
        let p1 = Peer::new("10.0.0.1:8080");
        let p2 = Peer::new("10.0.0.2:8080");
        state = add_peers(state, std::slice::from_ref(&p1)).0;
        state = add_peers(state, std::slice::from_ref(&p2)).0;

        let effect = run_update(Event::RemovePeers(vec![p1.clone()]), &mut state).await;

        assert!(!state.peers.contains(&p1));
        assert!(state.peers.contains(&p2));
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn add_transaction_rejects_invalid_recipient() {
        let mut state = funded_state();
        let invalid = Address {
            der: "this-is-not-hex".to_string(),
        };
        let previous_state = state.clone();

        let effect = run_update(Event::AddTransaction(invalid, 10, 0), &mut state).await;

        assert_eq!(effect, Effect::None);
        assert_eq!(state, previous_state);
    }

    #[tokio::test]
    async fn add_transaction_accepts_and_broadcasts_when_valid() {
        let mut state = funded_state();
        let (recipient, _) = keypair();

        let effect = run_update(Event::AddTransaction(recipient, 10, 0), &mut state).await;

        assert_eq!(state.transactions.len(), 1);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponseTransactions(state.transactions.clone()))
        );
    }

    #[tokio::test]
    async fn add_transaction_with_fee_is_rejected_when_not_enough_for_fee() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let previous_state = state.clone();

        let effect = run_update(Event::AddTransaction(recipient, 50, 1), &mut state).await;

        assert_eq!(state, previous_state);
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn add_transaction_with_fee_is_broadcast_when_sufficient() {
        let mut state = funded_state();
        let (recipient, _) = keypair();

        let effect = run_update(Event::AddTransaction(recipient, 48, 2), &mut state).await;

        assert_eq!(state.transactions.len(), 1);
        assert_eq!(state.transactions[0].fee, 2);
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
            signature: SignatureWrapper::default(),
        };
        let tx2 = Transaction {
            sender: state.address.clone(),
            out: Vec::new(),
            tx_in: Vec::new(),
            fee: 3,
            signature: SignatureWrapper::default(),
        };
        state.transactions = vec![tx1, tx2];

        let effect = run_update(Event::MineBlock, &mut state).await;

        assert!(state.transactions.is_empty());
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
                signature: SignatureWrapper::default(),
            })
            .collect();
        state.transactions = txs;

        let effect = run_update(Event::MineBlock, &mut state).await;

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
        let previous_state = state.clone();

        let effect = run_update(
            Event::P2PMessage(None, P2PMessage::QueryTransactions),
            &mut state,
        )
        .await;

        assert_eq!(state, previous_state);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponseTransactions(expected))
        );
    }

    #[tokio::test]
    async fn response_transactions_adds_new_and_rebroadcasts() {
        let mut state = funded_state();
        let (recipient, _) = keypair();
        let tx = build_tx(&state, &recipient, 10, 0);

        let effect = run_update(
            Event::P2PMessage(None, P2PMessage::ResponseTransactions(vec![tx.clone()])),
            &mut state,
        )
        .await;

        assert_eq!(state.transactions, vec![tx.clone()]);
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
        let previous_state = state.clone();

        let effect = run_update(
            Event::P2PMessage(None, P2PMessage::ResponseTransactions(vec![tx])),
            &mut state,
        )
        .await;

        assert_eq!(state, previous_state);
        assert_eq!(effect, Effect::None);
    }

    #[tokio::test]
    async fn query_peers_adds_sender_and_responds_with_known_peers() {
        let mut state = funded_state();
        let existing = Peer::new("10.0.0.1:8080");
        state = add_peers(state, std::slice::from_ref(&existing)).0;
        let sender = Peer::new("10.0.0.2:8080");

        let effect = run_update(
            Event::P2PMessage(Some(sender.clone()), P2PMessage::QueryPeers),
            &mut state,
        )
        .await;

        assert!(state.peers.contains(&existing));
        assert!(state.peers.contains(&sender));
        assert_eq!(state.peers.len(), 2);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponsePeers(vec![existing]))
        );
    }

    #[tokio::test]
    async fn query_peers_without_sender_returns_current_list() {
        let mut state = funded_state();
        let existing = Peer::new("10.0.0.1:8080");
        state = add_peers(state, std::slice::from_ref(&existing)).0;
        let previous_state = state.clone();

        let effect = run_update(Event::P2PMessage(None, P2PMessage::QueryPeers), &mut state).await;

        assert_eq!(state, previous_state);
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponsePeers(vec![existing]))
        );
    }

    #[tokio::test]
    async fn response_peers_merges_and_rebroadcasts() {
        let mut state = funded_state();
        let existing = Peer::new("10.0.0.1:8080");
        let new_peer = Peer::new("10.0.0.2:8080");
        state = add_peers(state, std::slice::from_ref(&existing)).0;

        let effect = run_update(
            Event::P2PMessage(None, P2PMessage::ResponsePeers(vec![new_peer.clone()])),
            &mut state,
        )
        .await;

        assert!(state.peers.contains(&existing));
        assert!(state.peers.contains(&new_peer));
        assert_eq!(
            effect,
            Effect::Broadcast(P2PMessage::ResponsePeers(state.peers.clone()))
        );
    }

    #[tokio::test]
    async fn response_peers_duplicate_is_ignored() {
        let mut state = funded_state();
        let existing = Peer::new("10.0.0.1:8080");
        state = add_peers(state, std::slice::from_ref(&existing)).0;
        let previous_state = state.clone();

        let effect = run_update(
            Event::P2PMessage(None, P2PMessage::ResponsePeers(vec![existing.clone()])),
            &mut state,
        )
        .await;

        assert_eq!(state, previous_state);
        assert_eq!(effect, Effect::None);
    }
}
