use serde::{Deserialize, Serialize};

use crate::{
    blockchain::address::{Address, is_valid_address},
    util::{key::SK, signature::SignatureWrapper},
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TransactionOut {
    pub address: Address,
    pub amount: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TransactionIn {
    pub unspent_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Transaction {
    pub sender: Address,
    pub out: Vec<TransactionOut>,
    pub tx_in: Vec<TransactionIn>,
    pub fee: u64,
    pub signature: SignatureWrapper,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} -> {}: {}",
            self.sender,
            self.out
                .iter()
                .map(|txout| txout.address.der.clone())
                .collect::<Vec<_>>()
                .join(", "),
            self.total_amount()
        )?;
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

impl Transaction {
    pub fn new(
        sender: Address,
        out: Vec<TransactionOut>,
        tx_in: Vec<TransactionIn>,
        fee: u64,
        signature: SignatureWrapper,
    ) -> Self {
        Self {
            sender,
            out,
            tx_in,
            fee,
            signature,
        }
    }
    pub fn new_with_creating_signature(
        sender: &Address,
        out: Vec<TransactionOut>,
        tx_in: Vec<TransactionIn>,
        fee: u64,
        sk: &SK,
    ) -> Self {
        let signature = create_transaction_signature(sender, &out, &tx_in, fee, sk);
        Self {
            sender: sender.clone(),
            out,
            tx_in,
            fee,
            signature,
        }
    }
    pub fn verify_signature(&self) -> bool {
        self.sender.verify(
            transaction_to_buf_for_signature(&self.sender, &self.out, &self.tx_in, self.fee)
                .as_slice(),
            &self.signature,
        )
    }

    /*
     * This method calculates the total amount of the transaction output.
     */
    pub fn total_amount(&self) -> u64 {
        self.fee + self.out.iter().map(|txout| txout.amount).sum::<u64>()
    }

    /*
     * This method calculates the total amount of the transaction input.
     */
    fn calc_total_input_amount(&self, unspent_transactions: &[UnspentTransaction]) -> u64 {
        self.tx_in
            .iter()
            .flat_map(|tx_in| tx_in.get_amount(unspent_transactions))
            .sum::<u64>()
    }

    pub fn get_unspent_transactions(
        &self,
        (previous_unspent, first_id): (Vec<UnspentTransaction>, u64),
    ) -> (Vec<UnspentTransaction>, u64 /*new id */) {
        let (mut new_unspent, new_id) =
            self.out
                .iter()
                .fold((previous_unspent, first_id), |(mut acc, id), tx_out| {
                    let (unspent, new_id) = tx_out.to_unspent(id);
                    acc.push(unspent);
                    (acc, new_id)
                });
        new_unspent.retain(|unspent| {
            !self
                .tx_in
                .iter()
                .any(|tx_in| tx_in.unspent_id == unspent.id)
        });
        (new_unspent, new_id)
    }

    pub fn fee_to_unspent_transaction(
        &self,
        miner: Address,
        (previous_unspent, first_id): (Vec<UnspentTransaction>, u64),
    ) -> (Vec<UnspentTransaction>, u64) {
        let fee_unspent = UnspentTransaction {
            id: first_id,
            address: miner,
            amount: self.fee,
        };
        (
            previous_unspent
                .iter()
                .chain([fee_unspent].iter())
                .cloned()
                .collect(),
            first_id + 1,
        )
    }

    pub fn is_valid(&self, unspent_transactions: &[UnspentTransaction]) -> bool {
        self.verify_signature()
            && self.total_amount() > 0
            && is_valid_address(&self.sender)
            && self
                .out
                .iter()
                .all(|txout| is_valid_address(&txout.address))
            && self.calc_total_input_amount(unspent_transactions) == self.total_amount()
            && self
                .tx_in
                .iter()
                .all(|tx_in| tx_in.get_amount(unspent_transactions).is_some())
    }
}

fn transaction_to_buf_for_signature(
    sender: &Address,
    out: &[TransactionOut],
    tx_in: &[TransactionIn],
    fee: u64,
) -> Vec<u8> {
    format!("{}{:?}{:?}{}", sender, out, tx_in, fee)
        .as_bytes()
        .to_vec()
}

fn create_transaction_signature(
    sender: &Address,
    out: &[TransactionOut],
    tx_in: &[TransactionIn],
    fee: u64,
    sk: &SK,
) -> SignatureWrapper {
    let data = transaction_to_buf_for_signature(sender, out, tx_in, fee);
    sk.sign(&data)
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

/*
 * COINBASE_AMOUNT_HALVING_INTERVAL and INITIAL_COINBASE_AMOUNT are subject to change in the future.
 */
const COINBASE_AMOUNT_HALVING_INTERVAL: u64 = 210000;
const INITIAL_COINBASE_AMOUNT: u64 = 50;
fn coinbase_amount(block_height: u64) -> u64 {
    let halvings: u64 = block_height / COINBASE_AMOUNT_HALVING_INTERVAL;
    INITIAL_COINBASE_AMOUNT >> halvings
}

fn coinbase_address() -> Address {
    Address { der: String::new() }
}
pub fn coinbase_transaction(address: &Address, block_height: u64) -> Transaction {
    Transaction {
        sender: coinbase_address(),
        out: vec![TransactionOut {
            address: address.clone(),
            amount: coinbase_amount(block_height),
        }],
        tx_in: Vec::new(),
        fee: 0,
        signature: SignatureWrapper::default(),
    }
}

pub fn is_valid_coinbase_transaction(transaction: &Transaction, block_height: u64) -> bool {
    transaction.sender == coinbase_address()
        && transaction.tx_in.is_empty()
        && transaction.out.len() == 1
        && transaction.out[0].amount == coinbase_amount(block_height)
        && transaction
            .out
            .iter()
            .all(|txout| is_valid_address(&txout.address))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::key::generate_sk;

    fn keypair() -> (Address, SK) {
        let sk = generate_sk(512);
        let pk = sk.to_pk();
        (pk, sk)
    }

    #[test]
    fn new_with_signature_creates_verifiable_tx() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();

        let tx = Transaction::new_with_creating_signature(
            &sender,
            vec![TransactionOut {
                address: recipient,
                amount: 10,
            }],
            vec![TransactionIn { unspent_id: 1 }],
            0,
            &sk,
        );

        let unspent_transactions = vec![UnspentTransaction {
            id: 1,
            address: sender,
            amount: 10,
        }];

        assert!(tx.verify_signature());
        assert!(tx.is_valid(&unspent_transactions));
    }

    #[test]
    fn verify_signature_fails_after_tamper() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();

        let mut tx = Transaction::new_with_creating_signature(
            &sender,
            vec![TransactionOut {
                address: recipient,
                amount: 10,
            }],
            vec![TransactionIn { unspent_id: 1 }],
            0,
            &sk,
        );

        let unspent_transactions = vec![UnspentTransaction {
            id: 1,
            address: sender,
            amount: 10,
        }];

        tx.out[0].amount = 11;
        assert!(!tx.verify_signature());
        assert!(!tx.is_valid(&unspent_transactions));
    }

    #[test]
    fn total_amount_sums_outputs() {
        let (sender, sk) = keypair();
        let (r1, _) = keypair();
        let (r2, _) = keypair();

        let tx = Transaction::new_with_creating_signature(
            &sender,
            vec![
                TransactionOut {
                    address: r1,
                    amount: 7,
                },
                TransactionOut {
                    address: r2,
                    amount: 13,
                },
            ],
            vec![TransactionIn { unspent_id: 1 }],
            3,
            &sk,
        );

        assert_eq!(tx.total_amount(), 23);
    }

    #[test]
    fn get_unspent_transactions_adds_outputs_and_consumes_inputs() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();

        let tx = Transaction::new_with_creating_signature(
            &sender,
            vec![
                TransactionOut {
                    address: recipient,
                    amount: 10,
                },
                TransactionOut {
                    address: sender.clone(),
                    amount: 5,
                },
            ],
            vec![TransactionIn { unspent_id: 1 }],
            0,
            &sk,
        );

        let prev = vec![
            UnspentTransaction {
                id: 1,
                address: sender.clone(),
                amount: 20,
            },
            UnspentTransaction {
                id: 2,
                address: sender,
                amount: 30,
            },
        ];

        let (next, new_id) = tx.get_unspent_transactions((prev, 3));

        assert_eq!(new_id, 5);
        assert!(next.iter().all(|u| u.id != 1));
        assert!(next.iter().any(|u| u.id == 2));
        assert!(next.iter().any(|u| u.id == 3));
        assert!(next.iter().any(|u| u.id == 4));
    }

    #[test]
    fn coinbase_transaction_is_valid() {
        let (miner, _) = keypair();
        let block_height = 1;
        let tx = coinbase_transaction(&miner, block_height);
        assert!(is_valid_coinbase_transaction(&tx, block_height));
    }

    #[test]
    fn coinbase_transaction_invalid_when_amount_changed() {
        let (miner, _) = keypair();
        let block_height = 1;
        let mut tx = coinbase_transaction(&miner, block_height);
        tx.out[0].amount = 999;
        assert!(!is_valid_coinbase_transaction(&tx, block_height));
    }

    #[test]
    fn get_transaction_out_returns_recipient_and_change() {
        let (sender, _) = keypair();
        let (recipient, _) = keypair();

        let out = get_transaction_out(&sender, &recipient, 30, 10, 100);

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].address, recipient);
        assert_eq!(out[0].amount, 30);
        assert_eq!(out[1].address, sender);
        assert_eq!(out[1].amount, 60);
    }

    #[test]
    fn flex_unspent_transactions_picks_minimum_prefix_to_reach_target() {
        let (addr, _) = keypair();

        let utxos = vec![
            UnspentTransaction {
                id: 1,
                address: addr.clone(),
                amount: 3,
            },
            UnspentTransaction {
                id: 2,
                address: addr.clone(),
                amount: 4,
            },
            UnspentTransaction {
                id: 3,
                address: addr,
                amount: 10,
            },
        ];

        let selected = flex_unspent_transactions(7, utxos.clone());
        assert_eq!(
            selected.iter().map(|u| u.id).collect::<Vec<_>>(),
            vec![1, 2]
        );

        let selected_insufficient = flex_unspent_transactions(100, utxos);
        assert_eq!(selected_insufficient.len(), 0);
    }

    #[test]
    fn is_invalid_when_input_output_amounts_do_not_match() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();

        let tx = Transaction::new_with_creating_signature(
            &sender,
            vec![TransactionOut {
                address: recipient,
                amount: 10,
            }],
            vec![TransactionIn { unspent_id: 1 }],
            0,
            &sk,
        );

        let unspent_transactions = vec![UnspentTransaction {
            id: 1,
            address: sender,
            amount: 9,
        }];

        assert!(tx.verify_signature());
        assert!(!tx.is_valid(&unspent_transactions));
    }

    #[test]
    fn verify_signature_fails_when_fee_is_tampered() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();

        let mut tx = Transaction::new_with_creating_signature(
            &sender,
            vec![TransactionOut {
                address: recipient,
                amount: 10,
            }],
            vec![TransactionIn { unspent_id: 1 }],
            2,
            &sk,
        );

        let unspent_transactions = vec![UnspentTransaction {
            id: 1,
            address: sender,
            amount: 12, // 10 + fee 2
        }];

        assert!(tx.verify_signature());
        assert!(tx.is_valid(&unspent_transactions));

        tx.fee = 3;
        assert!(!tx.verify_signature());
        assert!(!tx.is_valid(&unspent_transactions));
    }

    #[test]
    fn is_valid_requires_input_to_equal_outputs_plus_fee() {
        let (sender, sk) = keypair();
        let (recipient, _) = keypair();

        let tx = Transaction::new_with_creating_signature(
            &sender,
            vec![TransactionOut {
                address: recipient,
                amount: 10,
            }],
            vec![TransactionIn { unspent_id: 1 }],
            2,
            &sk,
        );

        let ok = vec![UnspentTransaction {
            id: 1,
            address: sender.clone(),
            amount: 12,
        }];
        let ng = vec![UnspentTransaction {
            id: 1,
            address: sender,
            amount: 11,
        }];

        assert!(tx.is_valid(&ok));
        assert!(!tx.is_valid(&ng));
    }
}
