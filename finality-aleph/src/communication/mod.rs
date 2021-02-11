mod gossip;
pub(super) mod peer;

use crate::{
    communication::gossip::GossipValidator,
    temp::{EpochId, NodeIndex, Round, Unit},
    AuthorityId, AuthoritySignature,
};
use codec::{Decode, Encode};
use log::debug;
use sc_network_gossip::{GossipEngine, Network};
use sp_application_crypto::RuntimeAppPublic;
use sp_runtime::traits::{Block, Hash, Header};
use std::sync::{Arc, Mutex};

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub struct NetworkBridge<B: Block, N: Network<B>> {
    service: N,
    gossip_engine: Arc<Mutex<GossipEngine<B>>>,
    validator: Arc<GossipValidator<B>>,
}

#[derive(Debug, Encode, Decode)]
struct SignedCHUnit<B: Block> {
    unit: CHUnit<B>,
    signature: AuthoritySignature,
    id: AuthorityId,
}

pub fn encode_unit_with_buffer<B: Block>(unit: &CHUnit<B>, buf: &mut Vec<u8>) {
    buf.clear();
    unit.encode_to(buf);
}

pub fn encode_unit<B: Block>(unit: &CHUnit<B>) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_unit_with_buffer(unit, &mut buf);
    buf
}

pub fn verify_unit_signature_with_buffer<B: Block>(
    unit: &CHUnit<B>, signature: &AuthoritySignature, id: &AuthorityId,
    buf: &mut Vec<u8>,
) -> bool {
    encode_unit_with_buffer(&unit, buf);

    let valid = id.verify(&buf, signature);
    if !valid {
        debug!(target: "afa", "Bad signature message from {:?}", unit.creator());
    }

    valid
}

pub fn verify_unit_signature<B: Block>(unit: &CHUnit<B>, signature: &AuthoritySignature, id: &AuthorityId) -> bool {
    verify_unit_signature_with_buffer(unit, signature, id, &mut Vec::new())
}

pub(crate) fn multicast_topic<B: Block>(round: Round, epoch: EpochId) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}-{}", round, epoch).as_bytes())
}

pub(crate) fn index_topic<B: Block>(index: NodeIndex) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}", index).as_bytes())
}
