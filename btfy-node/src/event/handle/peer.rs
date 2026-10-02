use crate::{
    effect::{Effect, when_changed},
    p2p::{P2PMessage, Peer},
    state::State,
};

pub fn handle_add_peer(state: &mut State, new_peer: Peer) -> Effect {
    let added = state.add_peers(&[new_peer]);
    when_changed(Effect::Broadcast(P2PMessage::QueryPeers), added)
}

pub fn handle_remove_peers(state: &mut State, remove_peers: Vec<Peer>) -> Effect {
    let remove_peers_ip = remove_peers.iter().map(|peer| peer.addr.to_string());
    if !remove_peers.is_empty() {
        debug!("remove peers: {:?}", remove_peers_ip);
    }
    state.peers.retain(|peer| !remove_peers.contains(peer));
    Effect::None
}
