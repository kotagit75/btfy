use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::blockchain::{address::Address, transaction::Transaction};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Encode, Decode)]
pub struct TransactionOut {
    pub address: Address,
    pub amount: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Encode, Decode)]
pub struct TransactionIn {
    pub unspent_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Encode, Decode)]
pub struct UnspentTransaction {
    pub id: u64,
    pub address: Address,
    pub amount: u64,
}
impl std::fmt::Display for TransactionOut {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.address, self.amount)?;
        Ok(())
    }
}

impl TransactionOut {
    pub fn to_unspent(&self, id: u64) -> (UnspentTransaction, u64 /*new id */) {
        (
            UnspentTransaction {
                id,
                address: self.address.clone(),
                amount: self.amount,
            },
            id + 1,
        )
    }
}

impl TransactionIn {
    pub fn get_amount(&self, unspent_transactions: &[UnspentTransaction]) -> Option<u64> {
        unspent_transactions
            .iter()
            .find(|ut| ut.id == self.unspent_id)
            .map(|ut| ut.amount)
    }
}

impl UnspentTransaction {
    pub fn to_txin(&self) -> TransactionIn {
        TransactionIn {
            unspent_id: self.id,
        }
    }
}

pub fn get_transaction_out(
    sender: &Address,
    recipient: &Address,
    send_amount: u64,
    fee: u64,
    unspent_amount: u64,
) -> Vec<TransactionOut> {
    vec![
        TransactionOut {
            address: recipient.clone(),
            amount: send_amount,
        },
        TransactionOut {
            address: sender.clone(),
            amount: unspent_amount - send_amount - fee,
        },
    ]
}

pub fn flex_unspent_transactions(
    target_amount: u64,
    unspent_transactions: Vec<UnspentTransaction>,
) -> Vec<UnspentTransaction> {
    let (amount, unspent) = unspent_transactions.iter().fold(
        (0, Vec::new()),
        |(amount, use_unspent), unspent_transaction| {
            if amount >= target_amount {
                return (amount, use_unspent);
            }
            (
                amount + unspent_transaction.amount,
                use_unspent
                    .iter()
                    .chain([unspent_transaction])
                    .cloned()
                    .collect(),
            )
        },
    );
    if amount < target_amount {
        return Vec::new();
    }
    unspent
}

pub fn transaction_to_unspent_ids(transaction: &Transaction) -> Vec<u64> {
    transaction
        .tx_in
        .iter()
        .map(|i| i.unspent_id)
        .collect::<Vec<_>>()
}

pub fn transactions_to_unspent_ids(transactions: &[Transaction]) -> Vec<u64> {
    transactions
        .iter()
        .flat_map(|tx| &tx.tx_in)
        .map(|i| i.unspent_id)
        .collect::<Vec<_>>()
}
