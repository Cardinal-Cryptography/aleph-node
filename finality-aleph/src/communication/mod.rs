mod gossip;
pub(super) mod peer;

use crate::{communication::gossip::GossipValidator, temp::EpochId, AuthoritySignature, AuthorityId};
use sc_network_gossip::{GossipEngine, Network};
use sp_runtime::traits::{Block, Hash, Header};
use std::{
    fmt::{Formatter, Result as FmtResult},
    sync::{Arc, Mutex},
};
use crate::temp::CHUnit;
use codec::{Encode, Decode};
use sp_application_crypto::RuntimeAppPublic;
use log::debug;

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub struct NetworkBridge<B: Block, N: Network<B>> {
    service: N,
    gossip_engine: Arc<Mutex<GossipEngine<B>>>,
    validator: Arc<GossipValidator<B>>,
}

struct SignedCHUnit<H> {
    unit: CHUnit<H>,
    signature: AuthoritySignature,
    id: AuthorityId,
}

pub fn encode_unit_with_buffer<H>(unit: &CHUnit<H>, buf: &mut Vec<u8>) {
    buf.clear();
    unit.encode_to(buf);
}

pub fn encode_unit<H>(unit: &CHUnit<H>) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_unit_with_buffer(unit, &mut buf);
    buf
}

pub fn verify_unit_signature_with_buffer<H>(
    signed_ch_unit: &SignedCHUnit<H>,
    buf: &mut Vec<u8>,
) -> bool {
    encode_unit_with_buffer(&signed_ch_unit.unit, buf);

    let valid = signed_ch_unit.id.verify(&buf, &signed_ch_unit.signature);
    if !valid {
        debug!(target: "afa", "Bad signature message from {:?}", signed_ch_unit.unit.creator_id);
    }

    valid
}

pub fn verify_unit_signature<H>(
    signed_ch_unit: &SignedCHUnit<H>
) -> bool {
    verify_unit_signature_with_buffer(signed_ch_unit, &mut Vec::new())
}


// TODO: remove if verified it won't be use as the protocol is async.
// pub(crate) fn global_topic<B: Block>(epoch: EpochId) -> B::Hash {
//     <<B::Header as Header>::Hashing as Hash>::hash(format!("{}-GLOBAL", epoch).as_bytes())
// }
