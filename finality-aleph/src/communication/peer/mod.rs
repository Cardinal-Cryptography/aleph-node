use sc_network::{ObservedRole, PeerId};
use sp_runtime::traits::Block;
use std::collections::HashMap;

pub(crate) mod rep;

pub struct PeerInfo {
    role: ObservedRole,
}

impl PeerInfo {
    fn new(role: ObservedRole) -> Self {
        PeerInfo { role }
    }
}

pub(crate) struct Peers(HashMap<PeerId, PeerInfo>);

impl Peers {
    pub(crate) fn insert_peer(&mut self, who: PeerId, role: ObservedRole) {
        self.0.insert(who, PeerInfo::new(role));
    }

    pub(crate) fn remove_peer(&mut self, who: &PeerId) {
        self.0.remove(who.as_ref());
    }

    pub(crate) fn get_peer(&self, who: &PeerId) -> Option<&PeerInfo> {
        self.0.get(who.as_ref())
    }

    pub(crate) fn authorities(&self) -> usize {
        self.0
            .iter()
            .filter(|_, info| matches!(info.roles, ObservedRole::Authority))
            .count()
    }

    pub(crate) fn non_authorities(&self) -> usize {
        self.0
            .iter()
            .filter(|(_, info)| matches!(info.roles, ObservedRole::Full | ObservedRole::Light))
            .count()
    }

    pub(crate) fn shuffle(&mut self) {
        let mut peers = self.0.clone();
        peers.partial_shuffle(&mut rand::thread_rng(), self.0.len());
        peers.truncate(self.0.len());
        self.0.clear();
        self.0.extend(peers.into_iter());
    }
}
