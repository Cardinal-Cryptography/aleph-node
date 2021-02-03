mod gossip;
pub(super) mod peer;

use crate::communication::gossip::GossipValidator;
use sc_network_gossip::{GossipEngine, Network};
use sp_runtime::traits::{Block, Hash, Header, NumberFor, Zero};
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Epoch(pub u64);

impl Display for Epoch {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub struct NetworkBridge<B: Block, N: Network<B>> {
    service: N,
    gossip_engine: Arc<Mutex<GossipEngine<B>>>,
    validator: Arc<GossipValidator<B>>,
}

pub(crate) fn global_topic<B: Block>(epoch: Epoch) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}-GLOBAL", epoch).as_bytes())
}
