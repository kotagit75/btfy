use openssl::error::ErrorStack;
use serde::{Deserialize, Serialize};

use crate::{
    beacon::{Beacon, is_valid_beacon},
    blockchain::{
        address::Address,
        block::{Block, genesis_block, solve_block_vdf},
        transaction::{
            Transaction, TransactionIn, UnspentTransaction, flex_unspent_transactions,
            get_transaction_out,
        },
    },
    util::key::SK,
};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Chain {
    pub blocks: Vec<Block>,
}

impl Chain {
    pub fn new() -> Self {
        Self {
            blocks: vec![genesis_block()],
        }
    }

    pub fn get_latest_block(&self) -> Block {
        match self.blocks.last() {
            Some(block) => block.clone(),
            None => genesis_block(),
        }
    }

    pub fn generate_next_block(
        &self,
        sk: &SK,
        issuer: &Address,
        beacon: Beacon,
        transactions: Vec<Transaction>,
    ) -> Result<Block, ErrorStack> {
        let previous_block: Block = self.get_latest_block();
        let next_index: u64 = previous_block.index + 1;
        let next_timestamp: i64 = chrono::Utc::now().timestamp_millis();
        let vdf_solution = solve_block_vdf(
            next_index,
            next_timestamp,
            &transactions,
            beacon.clone(),
            issuer,
            previous_block.hash,
        )
        .unwrap();
        Block::new_with_creating_signature(
            next_index,
            next_timestamp,
            transactions,
            beacon,
            vdf_solution,
            issuer,
            previous_block.hash,
            sk,
        )
    }

    pub fn is_valid(&self) -> bool {
        let is_valid_genesis_block = self.blocks.first().cloned() == Some(genesis_block());
        let is_valid_chain = self
            .blocks
            .windows(2)
            .all(|windows| is_valid_new_block(&windows[0], &windows[1]));
        is_valid_genesis_block && is_valid_chain
    }

    pub fn replace(&self, new_chain: Chain) -> Self {
        if new_chain.is_valid() && new_chain.blocks.len() > self.blocks.len() {
            Self {
                blocks: new_chain.blocks,
            }
        } else {
            self.clone()
        }
    }

    pub fn add_block(&self, block: Block, i_generated: bool, generated_now: bool) -> (Self, bool) {
        if !i_generated
            && generated_now
            && !is_valid_beacon(&block.beacon, &self.get_latest_block().hash)
        {
            print!("aaa");
            return (self.clone(), false);
        }

        if is_valid_new_block(&block, &self.get_latest_block()) {
            (
                Self {
                    blocks: self
                        .blocks
                        .iter()
                        .chain(std::iter::once(&block))
                        .cloned()
                        .collect(),
                },
                true,
            )
        } else {
            (self.clone(), false)
        }
    }

    pub fn get_unspent_transactions(&self) -> (Vec<UnspentTransaction>, u64 /*new id */) {
        self.blocks.iter().fold((Vec::new(), 1), |acc, block| {
            block.get_unspent_transactions(acc)
        })
    }

    pub fn find_unspent_transaction(&self, unspent_id: u64) -> Option<UnspentTransaction> {
        let (unspent_transactions, _) = self.get_unspent_transactions();
        unspent_transactions
            .iter()
            .find(|unspent| unspent.id == unspent_id)
            .cloned()
    }

    pub fn generate_transaction(
        &self,
        sender: &Address,
        recipient: &Address,
        amount: u64,
        secret_key: &SK,
        used_transactions: &[Transaction],
    ) -> Result<Option<Transaction>, ErrorStack> {
        let (mut unspent_transactions, _) = self.get_unspent_transactions();

        let used_unspent_ids: Vec<u64> = used_transactions
            .iter()
            .flat_map(|tx| tx.tx_in.iter().map(|i| i.unspent_id))
            .collect();

        unspent_transactions.retain(|tx| !used_unspent_ids.iter().any(|t| *t == tx.id));

        let my_unspent_transactions: Vec<UnspentTransaction> = unspent_transactions
            .iter()
            .filter(|tx| &tx.address == sender)
            .cloned()
            .collect();
        let use_unspent = flex_unspent_transactions(amount, my_unspent_transactions);
        if use_unspent.is_empty() {
            return Ok(None);
        }
        let transaction = Transaction::new_with_creating_signature(
            sender,
            get_transaction_out(
                sender,
                recipient,
                amount,
                use_unspent.iter().map(|tx| tx.amount).sum(),
            ),
            use_unspent
                .iter()
                .map(|tx| TransactionIn { unspent_id: tx.id })
                .collect(),
            secret_key,
        )?;
        Ok(Some(transaction))
    }

    pub fn get_balance(&self, address: &Address) -> u64 {
        let (unspent_transactions, _) = self.get_unspent_transactions();
        unspent_transactions
            .iter()
            .filter(|tx| &tx.address == address)
            .map(|tx| tx.amount)
            .sum()
    }

    pub fn get_beacon_history(&self) -> Vec<Beacon> {
        self.blocks
            .iter()
            .map(|block| block.beacon.clone())
            .collect()
    }
}

pub fn is_valid_new_block(block: &Block, previous_block: &Block) -> bool {
    block.index == previous_block.index + 1
        && block.timestamp > previous_block.timestamp
        && block.previous_hash == previous_block.hash
        && block.calculate_hash() == block.hash
}
