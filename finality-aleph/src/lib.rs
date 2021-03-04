// TEMP allow as everything gets plugged into each other.
// TODO: Remove before we do a release to ensure there is no hanging code.
#![allow(dead_code)]
#![allow(clippy::type_complexity)]

use rush::Unit;
use rush::nodes::NodeIndex;
use rush::HashT;
use codec::{Codec, Encode, Decode};
use sp_runtime::traits::{Block as SubstrateBlock, Member, MaybeSerializeDeserialize, MaybeDisplay, SimpleBitOps, MaybeMallocSizeOf, MaybeSerialize};
use std::fmt::Debug;

pub(crate) mod communication;
pub mod config;
pub(crate) mod environment;

mod key_types {
    use sp_runtime::KeyTypeId;

    pub const ALEPH: KeyTypeId = KeyTypeId(*b"alph");
}

mod app {
    use crate::key_types::ALEPH;
    use sp_application_crypto::{app_crypto, ed25519};
    app_crypto!(ed25519, ALEPH);
}

pub type AuthorityId = app::Public;

pub type AuthoritySignature = app::Signature;

pub type AuthorityPair = app::Pair;

pub type Round = u64;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct UnitCoord {
    pub creator: NodeIndex,
    pub round: u64,
}

impl<B: HashT, H: HashT> From<Unit<B, H>> for UnitCoord {
    fn from(unit: Unit<B, H>) -> Self {
        UnitCoord {
            creator: unit.creator(),
            round: unit.round() as u64,
        }
    }
}

impl<B: HashT, H: HashT> From<&Unit<B, H>> for UnitCoord {
    fn from(unit: &Unit<B, H>) -> Self {
        UnitCoord {
            creator: unit.creator(),
            round: unit.round() as u64,
        }
    }
}

pub trait BlockExt {
    // type BlockHash: Member + MaybeSerializeDeserialize + Debug + std::hash::Hash + Ord
    // + Copy + MaybeDisplay + Default + SimpleBitOps + Codec + AsRef<[u8]> + AsMut<[u8]>
    // + MaybeMallocSizeOf
    type BlockHash: Member + MaybeSerializeDeserialize + Debug + std::hash::Hash + Ord
    + Copy + MaybeDisplay + Default + SimpleBitOps + Codec + AsRef<[u8]> + AsMut<[u8]>
    + MaybeMallocSizeOf;

    // type Thing: Codec + AsRef<[u8]> + AsMut<[u8]> + MaybeMallocSizeOf;

    fn best_block_hash(&self) -> Self::BlockHash;
}

pub trait Block: SubstrateBlock + BlockExt {}

/// Temporary structs and traits until initial version of Aleph is published.
pub(crate) mod temp {
    use codec::{Decode, Encode};

    #[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Encode, Decode)]
    pub struct NodeMap<T>(pub Vec<T>);

    impl<T> From<Vec<T>> for NodeMap<T> {
        fn from(vec: Vec<T>) -> Self {
            NodeMap(vec)
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
    pub struct ControlHash<H> {
        pub parents: NodeMap<bool>,
        pub hash: H,
    }
}
