use serde::{Deserialize, Serialize};

use crate::{
    blockchain::{
        address::Address,
        chain::Chain,
        transaction::{Transaction, transaction_to_unspent_ids, transactions_to_unspent_ids},
    },
    p2p::Peer,
    util::key::SK,
};

const MAX_PEERS: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub secret_key: SK,
    pub address: Address,
    pub chain: Chain,
    pub transactions: Vec<Transaction>,
    pub peers: Vec<Peer>,
}

impl State {
    pub fn new(secret_key: SK, chain: Chain) -> Self {
        let address = secret_key.to_pk();
        Self {
            secret_key,
            address,
            chain,
            transactions: Vec::new(),
            peers: Vec::new(),
        }
    }

    fn add_transaction_without_validation(&self, transaction: &Transaction) -> Self {
        Self {
            transactions: self
                .transactions
                .clone()
                .into_iter()
                .chain([transaction.clone()])
                .collect(),
            ..self.clone()
        }
    }

    pub fn add_transaction(&self, transaction: &Transaction) -> (Self, bool) {
        let tx_in_ids = transaction_to_unspent_ids(transaction);
        let state_tx_in_ids = transactions_to_unspent_ids(&self.transactions);

        let is_valid = transaction.is_valid(&self.chain.get_unspent_transactions().0);
        let inputs_exist =
            self.chain.find_unspent_transactions(&tx_in_ids).len() == tx_in_ids.len();
        let double_spent_in_pool = tx_in_ids.iter().any(|id| state_tx_in_ids.contains(id));

        if !is_valid || !inputs_exist || double_spent_in_pool {
            return (self.clone(), false);
        }

        (self.add_transaction_without_validation(transaction), true)
    }

    pub fn add_peer(&self, peer: &Peer) -> (Self, bool) {
        if self.peers.contains(peer) || self.peers.len() > MAX_PEERS {
            return (self.clone(), false);
        }
        (
            Self {
                peers: self
                    .peers
                    .clone()
                    .into_iter()
                    .chain([peer.clone()])
                    .collect(),
                ..self.clone()
            },
            true,
        )
    }

    pub fn add_peers(&self, peers: &[Peer]) -> (Self, bool) {
        peers
            .iter()
            .fold((self.clone(), false), |(state, changed), peer| {
                let (state, changed_) = state.add_peer(peer);
                (state, changed || changed_)
            })
    }

    pub fn remove_peer(&self, peer: &Peer) -> (Self, bool) {
        if !self.peers.contains(peer) {
            return (self.clone(), false);
        }
        (
            Self {
                peers: self
                    .peers
                    .clone()
                    .into_iter()
                    .filter(|p| p != peer)
                    .collect(),
                ..self.clone()
            },
            true,
        )
    }

    pub fn remove_peers(&self, peers: &[Peer]) -> (Self, bool) {
        peers
            .iter()
            .fold((self.clone(), false), |(state, changed), peer| {
                let (state, changed_) = state.remove_peer(peer);
                (state, changed || changed_)
            })
    }
}
