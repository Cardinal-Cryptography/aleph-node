use current_aleph_bft::NodeIndex;
use parity_scale_codec::Codec;
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

pub mod data;
mod gossip;
#[cfg(test)]
pub mod mock;
pub mod session;
mod substrate;
pub mod tcp;

#[cfg(test)]
pub use gossip::mock::{MockEvent, MockRawNetwork};
pub use gossip::{
    Error as GossipError, Network as GossipNetwork, Protocol, Service as GossipService,
};
use network_clique::{AddressingInformation, NetworkIdentity, PeerId};
pub use substrate::{ProtocolNaming, SubstrateNetwork};

use crate::BlockId;

/// Abstraction for requesting stale blocks.
pub trait RequestBlocks: Clone + Send + Sync + 'static {
    /// Request the given block -- this is supposed to be used only for "old forks".
    fn request_stale_block(&self, block: BlockId);
}

/// A basic alias for properties we expect basic data to satisfy.
pub trait Data: Clone + Codec + Send + Sync + 'static {}

impl<D: Clone + Codec + Send + Sync + 'static> Data for D {}

#[derive(Debug, Clone)]
struct SingleValidatorNetworkDetails {
    address: String,
    network_level_peer_id: String,
    authority_index_in_current_session: Option<NodeIndex>,
}
pub struct ValidatorsAddressingInfo {
    data: Arc<Mutex<HashMap<String, SingleValidatorNetworkDetails>>>,
}

impl ValidatorsAddressingInfo {
    pub fn new() -> Self {
        ValidatorsAddressingInfo {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    fn update<A: AddressingInformation>(&self, info: A, network_level_peer_id: String) {
        self.data.lock().insert(
            info.peer_id().to_string(),
            SingleValidatorNetworkDetails {
                address: info.internal_protocol_address(),
                network_level_peer_id,
                authority_index_in_current_session: None,
            },
        );
    }
}
