use std::fmt::{self, Debug, Display};

use openssl::error::ErrorStack;
use serde::{Deserialize, Serialize};
use vdf::InvalidIterations;

use crate::{
    beacon::Beacon,
    blockchain::{
        address::Address,
        transaction::{Transaction, UnspentTransaction, is_valid_coinbase_transaction},
    },
    util::{
        hash::{Hashed, hash},
        key::{PK, SK},
        signature::Signature,
        vdf::{solve, verify_solution},
    },
};

pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 100;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transactions: Vec<Transaction>,
    pub beacon: Beacon,
    pub vdf_solution: Vec<u8>,
    pub previous_hash: Hashed,
    pub issuer: Address,
    pub signature: Signature,
    pub hash: Hashed,
}

impl Block {
    pub fn new(
        index: u64,
        timestamp: i64,
        transactions: Vec<Transaction>,
        beacon: Beacon,
        vdf_solution: Vec<u8>,
        issuer: &Address,
        previous_hash: Hashed,
        signature: Signature,
    ) -> Self {
        let hash = calculate_hash(
            &BlockData::new(
                index,
                timestamp,
                &transactions,
                &beacon,
                &issuer,
                previous_hash.clone(),
            ),
            &vdf_solution,
            signature.clone(),
        );
        Self {
            index,
            timestamp,
            transactions,
            beacon,
            vdf_solution,
            previous_hash,
            issuer: issuer.clone(),
            signature,
            hash,
        }
    }
    pub fn new_with_creating_signature(
        index: u64,
        timestamp: i64,
        transactions: Vec<Transaction>,
        beacon: Beacon,
        vdf_solution: Vec<u8>,
        issuer: &Address,
        previous_hash: Hashed,
        sk: &SK,
    ) -> Result<Self, ErrorStack> {
        Ok(Self::new(
            index,
            timestamp,
            transactions.clone(),
            beacon.clone(),
            vdf_solution.clone(),
            issuer,
            previous_hash,
            create_block_signature(
                &BlockData::new(
                    index,
                    timestamp,
                    &transactions,
                    &beacon,
                    &issuer,
                    previous_hash.clone(),
                ),
                &vdf_solution,
                sk,
            )?,
        ))
    }
    pub fn verify_signature(&self) -> bool {
        self.issuer.verify(
            block_to_buf_for_signature(&self.to_blockdata(), &self.vdf_solution).as_slice(),
            &self.signature,
        )
    }
    pub fn verify_vdf_solution(&self) -> bool {
        verify_solution(
            block_to_buf_for_vdf(&self.to_blockdata()).as_slice(),
            &self.vdf_solution,
        )
    }

    fn get_block_height(&self) -> u64 {
        self.index
    }

    pub fn is_valid(&self, unspent_transactions: &[UnspentTransaction]) -> bool {
        if self.transactions.len() > MAX_TRANSACTIONS_PER_BLOCK {
            return false;
        }
        if let Some((coinbase, normal)) = self.transactions.split_first() {
            self.verify_signature()
                && self.verify_vdf_solution()
                && is_valid_coinbase_transaction(coinbase, self.get_block_height())
                && normal.iter().all(|t| t.is_valid(unspent_transactions))
        } else {
            false
        }
    }

    fn to_blockdata(&self) -> BlockData<'_> {
        BlockData::new(
            self.index,
            self.timestamp,
            &self.transactions,
            &self.beacon,
            &self.issuer,
            self.previous_hash,
        )
    }

    pub fn calculate_hash(&self) -> Hashed {
        calculate_hash(
            &self.to_blockdata(),
            &self.vdf_solution,
            self.signature.clone(),
        )
    }

    pub fn get_unspent_transactions(
        &self,
        (previous_unspent, first_id): (Vec<UnspentTransaction>, u64),
    ) -> (Vec<UnspentTransaction>, u64 /*new id */) {
        self.transactions
            .iter()
            .fold((previous_unspent, first_id), |acc, tx| {
                tx.fee_to_unspent_transaction(self.issuer.clone(), tx.get_unspent_transactions(acc))
            })
    }
}

pub struct BlockData<'a> {
    index: u64,
    timestamp: i64,
    transactions: &'a [Transaction],
    beacon: &'a Beacon,
    issuer: &'a Address,
    previous_hash: Hashed,
}
impl<'a> BlockData<'a> {
    pub fn new(
        index: u64,
        timestamp: i64,
        transactions: &'a [Transaction],
        beacon: &'a Beacon,
        issuer: &'a Address,
        previous_hash: Hashed,
    ) -> Self {
        Self {
            index,
            timestamp,
            transactions,
            beacon,
            issuer,
            previous_hash,
        }
    }
}
impl<'a> Display for BlockData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{:?}{:?}{:?}{:?}",
            self.index,
            self.timestamp,
            self.transactions,
            self.beacon,
            self.issuer,
            self.previous_hash
        )
    }
}

pub fn calculate_hash(blockdata: &BlockData, vdf_solution: &[u8], signature: Signature) -> Hashed {
    hash(format!("{}{:?}{:?}", blockdata, vdf_solution, signature).as_bytes())
}

fn block_to_buf_for_signature(blockdata: &BlockData, vdf_solution: &[u8]) -> Vec<u8> {
    format!("{}{:?}", blockdata, vdf_solution)
        .as_bytes()
        .to_vec()
}

fn create_block_signature(
    blockdata: &BlockData,
    vdf_solution: &[u8],
    sk: &SK,
) -> Result<Signature, ErrorStack> {
    let data = block_to_buf_for_signature(blockdata, vdf_solution);
    sk.sign(&data)
}

const GENESIS_BLOCK_DATA: &str = include_str!("genesis.txt");
pub fn genesis_block() -> Block {
    let pk = PK {
        der: GENESIS_BLOCK_DATA.to_string(),
    };
    Block::new(
        0,
        0,
        Vec::new(),
        Beacon { values: Vec::new() },
        Vec::new(),
        &pk,
        [0; 32],
        Vec::new(),
    )
}

fn block_to_buf_for_vdf(blockdata: &BlockData) -> Vec<u8> {
    blockdata.to_string().as_bytes().to_vec()
}
pub fn solve_block_vdf(blockdata: &BlockData) -> Result<Vec<u8>, InvalidIterations> {
    solve(block_to_buf_for_vdf(blockdata).as_slice())
}

#[cfg(test)]
mod tests {
    use crate::{
        blockchain::transaction::{TransactionIn, TransactionOut, coinbase_transaction},
        util::key::generate_pk_and_sk,
    };

    use super::*;

    fn keypair() -> (Address, SK) {
        let (pk, sk) = generate_pk_and_sk(512).unwrap();
        (pk, sk)
    }

    #[test]
    fn get_unspent_transactions_adds_fee_to_miner_utxo() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();
        let (miner, _) = keypair();

        let tx = Transaction::new_with_creating_signature(
            &sender,
            vec![TransactionOut {
                address: recipient,
                amount: 10,
            }],
            vec![TransactionIn { unspent_id: 1 }],
            2,
            &sk,
        )
        .unwrap();

        let prev = vec![UnspentTransaction {
            id: 1,
            address: sender,
            amount: 12,
        }];

        let b = Block {
            index: 1,
            timestamp: 1,
            transactions: vec![coinbase_transaction(&miner, 1), tx],
            beacon: Beacon { values: vec![] },
            vdf_solution: vec![],
            previous_hash: [0; 32],
            issuer: miner.clone(),
            signature: vec![],
            hash: [1; 32],
        };

        let (next, _) = b.get_unspent_transactions((prev, 2));

        assert!(next.iter().any(|u| u.address == miner && u.amount == 2));
    }

    #[test]
    fn is_invalid_when_too_many_transactions() {
        let (miner, _) = keypair();
        let transactions = vec![coinbase_transaction(&miner, 1); MAX_TRANSACTIONS_PER_BLOCK + 1];

        let block = Block {
            index: 1,
            timestamp: 1,
            transactions,
            beacon: Beacon { values: vec![] },
            vdf_solution: vec![],
            previous_hash: [0; 32],
            issuer: miner,
            signature: vec![],
            hash: [1; 32],
        };

        assert!(!block.is_valid(&[]));
    }
}
