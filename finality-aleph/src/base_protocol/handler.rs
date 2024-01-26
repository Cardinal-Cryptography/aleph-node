use std::collections::{HashMap, HashSet};

use parity_scale_codec::DecodeAll;
use sc_network::{config::FullNetworkConfiguration, ExtendedPeerInfo, PeerId};
use sc_network_common::{role::Roles, sync::message::BlockAnnouncesHandshake};
use sp_runtime::traits::{Block, Header};

use crate::{BlockHash, BlockNumber};

struct PeerInfo {
    role: Roles,
}

/// Handler for the base protocol.
pub struct Handler<B>
where
    B: Block<Hash = BlockHash>,
    B::Header: Header<Number = BlockNumber>,
{
    peers: HashMap<PeerId, PeerInfo>,

    full_non_reserved_peers: HashSet<PeerId>,

    reserved_nodes: HashSet<PeerId>,
    reserved_peers: HashSet<PeerId>,

    // TODO check if it shouldn't count also reserved nodes
    max_full_non_reserved_peers: usize,
    max_light_peers: usize,
    max_inbound_peers: usize,
    num_inbound_peers: usize,

    genesis_hash: B::Hash,
}

pub enum ConnectionError {
    BadlyEncodedHandshake,
    BadHandshakeGenesis,
    PeerAlreadyConnected,
    TooManyFullPeers,
    TooManyFullInboundPeers,
    NonFullReservedNode,
    TooManyLightNodes,
}

impl<B> Handler<B>
where
    B: Block<Hash = BlockHash>,
    B::Header: Header<Number = BlockNumber>,
{
    /// Create a new handler.
    pub fn new(genesis_hash: B::Hash, net_config: &FullNetworkConfiguration) -> Self {
        let reserved_nodes = net_config
            .network_config
            .default_peers_set
            .reserved_nodes
            .iter()
            .map(|reserved| reserved.peer_id)
            .collect();

        let max_full_non_reserved_peers =
            net_config.network_config.default_peers_set_num_full as usize;

        let total = net_config.network_config.default_peers_set.out_peers
            + net_config.network_config.default_peers_set.in_peers;
        let max_light_peers =
            total.saturating_sub(net_config.network_config.default_peers_set_num_full) as usize;

        // default_peers_set.in_peers contains unwanted light nodes
        let max_inbound_peers = net_config
            .network_config
            .default_peers_set_num_full
            .saturating_sub(net_config.network_config.default_peers_set.out_peers)
            as usize;

        Handler {
            peers: HashMap::new(),
            full_non_reserved_peers: HashSet::new(),
            reserved_nodes,
            reserved_peers: HashSet::new(),
            max_full_non_reserved_peers,
            max_light_peers,
            max_inbound_peers,
            num_inbound_peers: 0,
            genesis_hash,
        }
    }

    /// Returns a list of connected peers with some additional information.
    // TODO(A0-3886): This shouldn't need to return the substrate type after replacing RPCs.
    // In particular, it shouldn't depend on `B`.
    pub fn peers_info(&self) -> Vec<(PeerId, ExtendedPeerInfo<B>)> {
        self.peers
            .iter()
            .map(|(id, info)| {
                (
                    *id,
                    ExtendedPeerInfo {
                        roles: info.role,
                        best_hash: Default::default(),
                        best_number: 0,
                    },
                )
            })
            .collect()
    }

    pub fn on_new_connection(
        &mut self,
        peer_id: PeerId,
        handshake: Vec<u8>,
        is_inbound: bool,
    ) -> Result<(), ConnectionError> {
        // verify connection
        let handshake = BlockAnnouncesHandshake::<B>::decode_all(&mut &handshake[..])
            .map_err(|_| ConnectionError::BadlyEncodedHandshake)?;
        if handshake.genesis_hash != self.genesis_hash {
            return Err(ConnectionError::BadHandshakeGenesis);
        }

        if self.peers.contains_key(&peer_id) {
            return Err(ConnectionError::PeerAlreadyConnected);
        }

        let is_reserved = self.reserved_nodes.contains(&peer_id);

        if is_reserved && !handshake.roles.is_full() {
            // we assume that reserved nodes must be full nodes
            return Err(ConnectionError::NonFullReservedNode);
        }

        if !is_reserved {
            // check slot constraints for full non-reserved nodes
            if handshake.roles.is_full()
                && self.full_non_reserved_peers.len() >= self.max_full_non_reserved_peers
            {
                return Err(ConnectionError::TooManyFullPeers);
            }
            if handshake.roles.is_full()
                && is_inbound
                && self.num_inbound_peers >= self.max_inbound_peers
            {
                return Err(ConnectionError::TooManyFullInboundPeers);
            }
            // check slot constraints for light nodes
            if handshake.roles.is_light()
                && self.peers.len() - self.full_non_reserved_peers.len() - self.reserved_nodes.len()
                    >= self.max_light_peers
            {
                return Err(ConnectionError::TooManyLightNodes);
            }
        }

        // update peer sets
        self.peers.insert(
            peer_id,
            PeerInfo {
                role: handshake.roles,
            },
        );

        if is_reserved {
            self.reserved_peers.insert(peer_id);
        } else if handshake.roles.is_full() {
            self.full_non_reserved_peers.insert(peer_id);
            if is_inbound {
                self.num_inbound_peers += 1;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{aleph_primitives::Block, base_protocol::handler::Handler};

    // TODO create some mock network config

    // #[test]
    // fn initially_no_peers() {
    //     let handler = Handler::<Block>::new();
    //     assert!(
    //         handler.peers_info().is_empty(),
    //         "there should be no peers initially"
    //     );
    // }
}
