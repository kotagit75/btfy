use openssl::error::ErrorStack;
use serde::{Deserialize, Serialize};

use crate::{
    beacon::{Beacon, is_valid_beacon},
    blockchain::{
        address::Address,
        block::{Block, BlockData, genesis_block, solve_block_vdf},
        transaction::{
            Transaction, TransactionIn, UnspentTransaction, coinbase_transaction,
            flex_unspent_transactions, get_transaction_out,
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
        transactions_without_coinbase: Vec<Transaction>,
    ) -> Result<Block, ErrorStack> {
        let previous_block: Block = self.get_latest_block();
        let next_index: u64 = previous_block.index + 1;
        let next_timestamp: i64 = chrono::Utc::now().timestamp_millis();
        let transactions = [coinbase_transaction(issuer, next_index)]
            .iter()
            .chain(&transactions_without_coinbase)
            .cloned()
            .collect::<Vec<Transaction>>();
        let vdf_solution = solve_block_vdf(&BlockData::new(
            next_index,
            next_timestamp,
            &transactions,
            &beacon,
            &issuer,
            previous_block.hash.clone(),
        ))
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
        let is_valid_chain = self.blocks.windows(2).all(|windows| {
            is_valid_new_block(&windows[0], &windows[1], &self.get_unspent_transactions().0)
        });
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
            && !is_valid_beacon(
                &block.beacon,
                &self.get_latest_block().hash,
                block.timestamp,
            )
        {
            return (self.clone(), false);
        }

        if is_valid_new_block(
            &block,
            &self.get_latest_block(),
            &self.get_unspent_transactions().0,
        ) {
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
        send_amount: u64,
        secret_key: &SK,
        used_transactions: &[Transaction],
        fee: u64,
    ) -> Result<Option<Transaction>, ErrorStack> {
        let amount = send_amount + fee;

        if self.get_balance(sender) < amount {
            return Ok(None);
        }

        let (mut unspent_transactions, _) = self.get_unspent_transactions();

        let used_unspent_ids: Vec<u64> = used_transactions
            .iter()
            .flat_map(|tx| tx.tx_in.iter().map(|i| i.unspent_id))
            .collect();

        unspent_transactions.retain(|tx| !used_unspent_ids.iter().any(|t| *t == tx.id));
        let sender_unspent_transactions: Vec<UnspentTransaction> = unspent_transactions
            .iter()
            .filter(|tx| &tx.address == sender)
            .cloned()
            .collect();
        let use_unspent = flex_unspent_transactions(amount, sender_unspent_transactions);
        if use_unspent.is_empty() {
            return Ok(None);
        }
        let transaction = Transaction::new_with_creating_signature(
            sender,
            get_transaction_out(
                sender,
                recipient,
                send_amount,
                fee,
                use_unspent.iter().map(|tx| tx.amount).sum::<u64>(),
            ),
            use_unspent
                .iter()
                .map(|tx| TransactionIn { unspent_id: tx.id })
                .collect(),
            fee,
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
}

pub fn is_valid_new_block(
    block: &Block,
    previous_block: &Block,
    unspent_transactions: &[UnspentTransaction],
) -> bool {
    block.index == previous_block.index + 1
        && block.timestamp > previous_block.timestamp
        && block.previous_hash == previous_block.hash
        && block.calculate_hash() == block.hash
        && block.is_valid(unspent_transactions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beacon::Beacon;
    use crate::blockchain::block::{Block, genesis_block};
    use crate::blockchain::transaction::{TransactionIn, coinbase_transaction};
    use crate::util::key::{SK, generate_pk_and_sk};

    fn keypair() -> (Address, SK) {
        let (pk, sk) = generate_pk_and_sk(512).unwrap();
        (pk, sk)
    }

    fn dummy_block(prev: &Block, txs: Vec<Transaction>, beacon: f32) -> Block {
        Block {
            index: prev.index + 1,
            timestamp: prev.timestamp + 1,
            transactions: txs,
            beacon: Beacon {
                values: vec![beacon],
            },
            vdf_solution: vec![],
            previous_hash: prev.hash,
            issuer: prev.issuer.clone(),
            signature: vec![],
            hash: [prev.index as u8 + 1; 32],
        }
    }

    fn chain_with_coinbase(miner: &Address) -> Chain {
        let g = genesis_block();
        let b1 = dummy_block(&g, vec![coinbase_transaction(miner, 1)], 1.0);
        Chain {
            blocks: vec![g, b1],
        }
    }

    #[test]
    fn new_has_only_genesis() {
        let c = Chain::new();
        assert_eq!(c.blocks.len(), 1);
        assert_eq!(c.get_latest_block(), genesis_block());
    }

    #[test]
    fn get_unspent_and_find_unspent_work() {
        let (miner, _) = keypair();
        let c = chain_with_coinbase(&miner);
        let (utxos, next_id) = c.get_unspent_transactions();
        assert_eq!(utxos.len(), 2); /* coinbase and fee */
        assert_eq!(utxos[0].amount, 50);
        assert_eq!(next_id, 3); /* coinbase -> fee ->  */
        assert!(c.find_unspent_transaction(1).is_some());
        assert!(c.find_unspent_transaction(999).is_none());
    }

    #[test]
    fn generate_transaction_returns_none_when_insufficient() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();
        let c = chain_with_coinbase(&sender);
        let tx = c
            .generate_transaction(&sender, &recipient, 999, &sk, &[], 0)
            .unwrap();
        assert!(tx.is_none());
    }

    #[test]
    fn generate_transaction_uses_utxo_and_returns_change() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();
        let c = chain_with_coinbase(&sender);

        let tx = c
            .generate_transaction(&sender, &recipient, 30, &sk, &[], 0)
            .unwrap()
            .unwrap();

        assert_eq!(tx.tx_in, vec![TransactionIn { unspent_id: 1 }]);
        assert_eq!(tx.out.iter().map(|o| o.amount).sum::<u64>(), 50);
    }

    #[test]
    fn generate_transaction_respects_used_transactions_filter() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();
        let c = chain_with_coinbase(&sender);

        let used = c
            .generate_transaction(&sender, &recipient, 30, &sk, &[], 0)
            .unwrap()
            .unwrap();

        let next = c
            .generate_transaction(&sender, &recipient, 10, &sk, &[used], 0)
            .unwrap();

        assert!(next.is_none());
    }

    #[test]
    fn get_balance_sums_unspent_by_address() {
        let (a, _) = keypair();
        let (b, _) = keypair();

        let g = genesis_block();
        let b1 = dummy_block(&g, vec![coinbase_transaction(&a, 0)], 1.0);
        let b2 = dummy_block(&b1, vec![coinbase_transaction(&b, 1)], 2.0);
        let c = Chain {
            blocks: vec![g, b1, b2],
        };

        assert_eq!(c.get_balance(&a), 50);
        assert_eq!(c.get_balance(&b), 50);
    }

    #[test]
    fn add_block_rejects_invalid_block() {
        let c = Chain::new();
        let bad = dummy_block(&c.get_latest_block(), vec![], 1.0);
        let (next, changed) = c.add_block(bad, false, false);
        assert!(!changed);
        assert_eq!(next, c);
    }

    #[test]
    fn replace_rejects_invalid_longer_chain() {
        let base = Chain::new();
        let g = genesis_block();
        let longer_but_invalid = Chain {
            blocks: vec![g.clone(), dummy_block(&g, vec![], 1.0)],
        };
        assert_eq!(base.replace(longer_but_invalid), base);
    }

    #[test]
    fn generate_transaction_returns_none_when_amount_plus_fee_exceeds_funds() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();
        let c = chain_with_coinbase(&sender);

        let tx = c
            .generate_transaction(&sender, &recipient, 49, &sk, &[], 2)
            .unwrap();

        assert!(tx.is_none());
    }
}
