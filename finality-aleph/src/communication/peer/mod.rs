use crate::{
    communication::{gossip::NeighborPacketV1, peer::rep::PeerMisbehavior},
    EpochId,
};
use log::trace;
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

    pub(crate) fn get_peer(&self, who: &PeerId) -> Option<&PeerInfo> {
        self.0.get(who.as_ref())
    }

    pub(crate) fn authorities(&self) -> usize {
        self.0
            .iter()
            .filter(|(_, info)| matches!(info.role, ObservedRole::Authority))
            .count()
    }

    pub(crate) fn non_authorities(&self) -> usize {
        self.0
            .iter()
            .filter(|(_, info)| matches!(info.role, ObservedRole::Full | ObservedRole::Light))
            .count()
    }

    /// Returns a new epoch id if the peer is known.
    // TODO: error
    pub(crate) fn update_peer(
        &mut self,
        who: &PeerId,
        update: NeighborPacketV1,
    ) -> Result<Option<EpochId>, PeerMisbehavior> {
        let peer = match self.0.get_mut(who.as_ref()) {
            None => return Ok(None),
            Some(p) => p,
        };

        if peer.epoch_id > update.epoch_id {
            return Err(PeerMisbehavior::InvalidEpochId);
        }

        peer.epoch_id = update.epoch_id;

        trace!(target: "afa", "Peer {} updated epoch. Now at {}", who, peer.epoch_id);

        Ok(Some(peer.epoch_id))
    }
}
