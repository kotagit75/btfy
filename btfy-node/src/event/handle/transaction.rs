use crate::{
    effect::{Effect, when_changed},
    p2p::P2PMessage,
    state::State,
};
use btfy_core::{address::Address, validation::is_valid_address};

pub fn handle_add_transaction(
    state: &mut State,
    recipient: &Address,
    send_amount: u64,
    fee: u64,
) -> Effect {
    if !is_valid_address(recipient) {
        info!("invalid recipient address: {}", recipient.der);
        return Effect::None;
    }
    if let Some(transaction) = state.chain.generate_transaction(
        &state.address,
        recipient,
        send_amount,
        &state.secret_key,
        &state.transactions,
        fee,
    ) {
        let changed = state.add_transaction_to_pool(&transaction);
        if changed {
            info!("added transaction: {:?}", transaction);
        } else {
            error!("failed to add transaction: {:?}", transaction);
        }
        when_changed(
            Effect::Broadcast(P2PMessage::ResponseTransactions(vec![transaction])),
            changed,
        )
    } else {
        Effect::None
    }
}
