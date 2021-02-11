use crate::EpochId;
use sc_network::{ObservedRole, PeerId};
use std::collections::HashMap;

pub(crate) mod rep;

#[derive(Debug, Clone)]
pub struct PeerInfo {
    epoch_id: EpochId,
    role: ObservedRole,
}

impl PeerInfo {
    fn new(role: ObservedRole) -> Self {
        PeerInfo {
            epoch_id: EpochId::default(),
            role,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Peers(HashMap<PeerId, PeerInfo>);

impl Peers {
    pub(crate) fn insert_peer(&mut self, who: PeerId, role: ObservedRole) {
        self.0.insert(who, PeerInfo::new(role));
    }

    pub(crate) fn remove_peer(&mut self, who: &PeerId) {
        self.0.remove(who.as_ref());
    }
}
