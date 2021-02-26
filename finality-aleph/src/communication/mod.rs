mod gossip;
pub(crate) mod peer;

use crate::{
    temp::{CreatorId, EpochId, NodeIndex, Round, Unit, UnitCoord, UnitCoords},
    AuthorityId, AuthoritySignature,
};
use codec::{Decode, Encode};
use log::debug;
use sp_application_crypto::RuntimeAppPublic;
use sp_runtime::{
    traits::{Block, Hash, Header},
    ConsensusEngineId,
};
use std::fmt::Write;

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub const ALEPH_ENGINE_ID: ConsensusEngineId = *b"ALPH";

pub const ALEPH_AUTHORITIES_KEY: &[u8] = b":aleph_authorities";

fn raw_topic(creator: &CreatorId, round: &Round) -> String {
    format!("{}-{}/", creator.0, round.0)
}

pub(crate) fn coord_topic<B: Block>(creator: &CreatorId, round: &Round) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(raw_topic(creator, round).as_bytes())
}

pub(crate) fn coords_topic<B: Block>(coords: &UnitCoords) -> B::Hash {
    let mut raw_topics = String::new();
    for coord in coords.iter() {
        write!(raw_topics, "{}", raw_topic(&coord.creator, &coord.round))
            .expect("Failed to write topic");
    }
    <<B::Header as Header>::Hashing as Hash>::hash(raw_topics.as_bytes())
}
