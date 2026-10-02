use btfy_core::block::{Block, MAX_TRANSACTIONS_PER_BLOCK};

use crate::{
    effect::{Effect, when_changed},
    p2p::P2PMessage,
    state::State,
};

pub fn handle_mine_block(state: &mut State) -> Effect {
    let mut sorted_transactions: Vec<_> = state.transactions.clone();
    sorted_transactions.sort_by_key(|tx| tx.fee);
    sorted_transactions.reverse();
    let (transactions_to_mine, remaining_transactions) = sorted_transactions.split_at(
        std::cmp::min(MAX_TRANSACTIONS_PER_BLOCK, sorted_transactions.len()),
    );

    state.transactions = remaining_transactions.to_vec();
    Effect::MineBlock(transactions_to_mine.to_vec())
}

pub fn handle_restart_mine_block(state: &mut State) -> Effect {
    let mut sorted_transactions: Vec<_> = state.transactions.clone();
    sorted_transactions.sort_by_key(|tx| tx.fee);
    sorted_transactions.reverse();
    let (transactions_to_mine, remaining_transactions) = sorted_transactions.split_at(
        std::cmp::min(MAX_TRANSACTIONS_PER_BLOCK, sorted_transactions.len()),
    );

    state.transactions = remaining_transactions.to_vec();
    Effect::MineBlock(transactions_to_mine.to_vec())
}

pub async fn handle_completed_mine_block(
    state: &mut State,
    new_block: Block,
    vdf_difficulty: u64,
) -> Effect {
    let changed = state
        .chain
        .add_block(new_block.clone(), None, vdf_difficulty);

    if changed {
        info!("added next block to chain");
    } else {
        error!("failed to add next block");
    }

    when_changed(
        Effect::Broadcast(P2PMessage::ResponseBlockChain(vec![new_block])),
        changed,
    )
}
