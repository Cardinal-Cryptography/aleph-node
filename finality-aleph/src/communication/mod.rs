mod gossip;
pub(super) mod peer;

use crate::{communication::gossip::GossipValidator, temp::EpochId};
use sc_network_gossip::{GossipEngine, Network};
use sp_runtime::traits::{Block, Hash, Header};
use std::{
    fmt::{Formatter, Result as FmtResult},
    sync::{Arc, Mutex},
};

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub struct NetworkBridge<B: Block, N: Network<B>> {
    service: N,
    gossip_engine: Arc<Mutex<GossipEngine<B>>>,
    validator: Arc<GossipValidator<B>>,
}

// TODO: remove if verified it won't be use as the protocol is async.
// pub(crate) fn global_topic<B: Block>(epoch: EpochId) -> B::Hash {
//     <<B::Header as Header>::Hashing as Hash>::hash(format!("{}-GLOBAL", epoch).as_bytes())
// }
