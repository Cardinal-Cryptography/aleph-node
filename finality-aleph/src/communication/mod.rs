mod gossip;
pub(super) mod peer;

use crate::{
    communication::gossip::{GossipValidator, PeerReport},
    temp::{EpochId, NodeIndex, Round, Unit},
    AuthorityId, AuthoritySignature,
};
use codec::{Decode, Encode};
use log::debug;
use prometheus_endpoint::Registry;
use sc_network_gossip::{GossipEngine, Network};
use sp_application_crypto::RuntimeAppPublic;
use sp_core::traits::BareCryptoStorePtr;
use sp_runtime::traits::{Block, Hash, Header};
use sp_utils::mpsc::TracingUnboundedReceiver;
use std::sync::{Arc, Mutex};
use sp_runtime::ConsensusEngineId;

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub const ALEPH_ENGINE_ID: ConsensusEngineId = *b"ALPH";

pub const ALEPH_AUTHORITIES_KEY: &[u8] = b":aleph_authorities";

#[derive(Debug, Encode, Decode)]
struct SignedUnit<B: Block> {
    unit: Unit<B>,
    signature: AuthoritySignature,
    // NOTE: This will likely be changed to a usize to get the authority out of
    // a map in the future to reduce data sizes of packets.
    id: AuthorityId,
}

impl<B: Block> SignedUnit<B> {
    pub(crate) fn encode_unit_with_buffer(&self, buf: &mut Vec<u8>) {
        buf.clear();
        self.unit.encode_to(buf);
    }

    pub fn verify_unit_signature_with_buffer(
        &self,
        buf: &mut Vec<u8>,
    ) -> bool {
        self.encode_unit_with_buffer(buf);

        let valid = self.id.verify(&buf, &self.signature);
        if !valid {
            debug!(target: "afa", "Bad signature message from {:?}", self.unit.creator);
        }

        valid
    }

    pub fn verify_unit_signature(&self) -> bool {
        self.verify_unit_signature_with_buffer(&mut Vec::new())
    }
}

pub(crate) fn multicast_topic<B: Block>(round: Round, epoch: EpochId) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}-{}", round, epoch).as_bytes())
}

pub(crate) fn index_topic<B: Block>(index: NodeIndex) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}", index).as_bytes())
}
