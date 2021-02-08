mod gossip;
pub(super) mod peer;

use crate::{
    communication::gossip::GossipValidator,
    temp::{CHUnit, EpochId, Round},
    AuthorityId, AuthoritySignature,
};
use codec::{Decode, Encode};
use log::debug;
use sc_network_gossip::{GossipEngine, Network};
use sp_application_crypto::RuntimeAppPublic;
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
    signed_ch_unit: &SignedCHUnit<B>,
    buf: &mut Vec<u8>,
) -> bool {
    encode_unit_with_buffer(&signed_ch_unit.unit, buf);

    let valid = signed_ch_unit.id.verify(&buf, &signed_ch_unit.signature);
    if !valid {
        debug!(target: "afa", "Bad signature message from {:?}", signed_ch_unit.unit.creator_id);
    }

    valid
}

pub fn verify_unit_signature<B: Block>(signed_ch_unit: &SignedCHUnit<B>) -> bool {
    verify_unit_signature_with_buffer(signed_ch_unit, &mut Vec::new())
}

pub(crate) fn topic<B: Block>(round: Round, epoch: EpochId) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}-{}", epoch, round).as_bytes())
}
