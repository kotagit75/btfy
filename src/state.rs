use openssl::error::ErrorStack;
use serde::{Deserialize, Serialize};

use crate::{
    blockchain::{address::Address, chain::Chain, transaction::Transaction},
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
    pub fn new(secret_key: SK, chain: Chain) -> Result<Self, ErrorStack> {
        secret_key.to_pk().map(|address| Self {
            secret_key,
            address,
            chain,
            transactions: Vec::new(),
            peers: Vec::new(),
        })
    }

    pub fn add_transaction(&self, transaction: &Transaction) -> (Self, bool) {
        if !(transaction.is_valid(&self.chain.get_unspent_transactions().0)
            && transaction.tx_in.iter().all(|tx_in| {
                self.chain
                    .find_unspent_transaction(tx_in.unspent_id)
                    .is_some()
            }))
        {
            return (self.clone(), false);
        }

        let tx_in_ids = transaction
            .tx_in
            .iter()
            .map(|tx_in| tx_in.unspent_id)
            .collect::<Vec<_>>();
        let state_tx_in_ids = self
            .transactions
            .iter()
            .flat_map(|t| t.tx_in.iter().map(|tx_in| tx_in.unspent_id))
            .collect::<Vec<_>>();
        if tx_in_ids.iter().any(|id| state_tx_in_ids.contains(id)) {
            return (self.clone(), false);
        }
        (
            Self {
                transactions: self
                    .transactions
                    .clone()
                    .into_iter()
                    .chain([transaction.clone()])
                    .collect(),
                ..self.clone()
            },
            true,
        )
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
