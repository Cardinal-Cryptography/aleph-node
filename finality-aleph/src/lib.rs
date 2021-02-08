pub(crate) mod communication;
pub(crate) mod environment;
pub(crate) mod nodes;

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

/// Temporary structs and traits until initial version of Aleph is published.
pub(crate) mod temp {
    use codec::{Decode, Encode};
    use sp_runtime::traits::{Hash, Block};
    use std::fmt::{Display, Formatter, Result as FmtResult};

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
    pub struct Round(pub u64);

    impl Display for Round {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{}", self.0)
        }
    }

    impl From<u64> for Round {
        fn from(id: u64) -> Self {
            Round(id)
        }
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
    pub struct EpochId(pub u64);

    impl Display for EpochId {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{}", self.0)
        }
    }

    impl From<u64> for EpochId {
        fn from(id: u64) -> Self {
            EpochId(id)
        }
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
    pub struct CreatorId(pub u64);

    impl From<u64> for CreatorId {
        fn from(id: u64) -> Self {
            CreatorId(id)
        }
    }

    #[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Encode, Decode)]
    pub struct NodeMap<T>(Vec<T>);

    #[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
    pub struct ControlHash<B: Block> {
        parents: NodeMap<bool>,
        hash: B::Hash,
    }

    #[derive(Debug, Encode, Decode)]
    pub struct CHUnit<B: Block> {
        creator: CreatorId,
        round: Round,
        epoch_id: EpochId,
        hash: B::Hash,
        control_hash: ControlHash<B>,
        best_block: B::Hash,
    }

    impl<B: Block> CHUnit<B> {
        pub fn creator(&self) -> CreatorId {
            self.creator
        }

        pub fn round(&self) -> Round {
            self.round
        }

        pub fn epoch(&self) -> EpochId {
            self.epoch_id
        }
    }

    pub struct Unit<H: Hash> {
        creator: CreatorId,
        round: u32,
        epoch_id: EpochId,
        hash: H,
        control_hash: ControlHash<H>,
        parents: NodeMap<Option<H>>,
        best_block: H,
    }
}

use temp::*;
