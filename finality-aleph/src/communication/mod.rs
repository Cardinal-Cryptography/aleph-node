mod gossip;
mod network;
pub(crate) mod peer;

use rush::{nodes::NodeIndex, EpochId, Round};
use sp_runtime::traits::{Block, Hash, Header};

pub const ALEPH_AUTHORITIES_KEY: &[u8] = b":aleph_authorities";

pub(crate) fn epoch_topic<B: Block>(epoch: EpochId) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}", epoch.0).as_bytes())
}

pub(crate) fn index_topic<B: Block>(index: NodeIndex) -> B::Hash {
    <<B::Header as Header>::Hashing as Hash>::hash(format!("{}", index.0).as_bytes())
}
