mod gossip;

use crate::communication::gossip::GossipValidator;
use sc_network_gossip::{GossipEngine, Network};
use sp_runtime::traits::{Block as BlockT, NumberFor, Zero};
use std::sync::{Arc, Mutex};

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

/// Cost scalars to be used when reporting peers.
mod cost {
    use sc_network::ReputationChange as Rep;
}

/// Benefit scalars used to report good peers.
mod benefit {
    use sc_network::ReputationChange as Rep;
}

pub struct NetworkBridge<B: BlockT, N: Network<B>> {
    service: N,
    gossip_engine: Arc<Mutex<GossipEngine<B>>>,
    validator: Arc<GossipValidator<B>>,
}
