use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    utxo::{TransactionIn, TransactionOut},
};
use btfy_util::{key::SK, signature::SignatureWrapper};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize, Encode, Decode)]
pub struct Transaction {
    pub sender: Address,
    pub out: Vec<TransactionOut>,
    pub tx_in: Vec<TransactionIn>,
    pub fee: u64,
    pub signature: SignatureWrapper,
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

#[cfg(test)]
mod tests {
    use btfy_util::key::generate_sk;

    use super::*;
    use crate::utxo::{UnspentTransaction, flex_unspent_transactions, get_transaction_out};

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
