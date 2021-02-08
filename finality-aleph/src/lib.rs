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

pub fn check_unit_signature_with_buffer<H, N>(
    unit: &
)

/// Temporary structs and traits until initial version of Aleph is published.
pub(crate) mod temp {
    use codec::{Decode, Encode};
    use sp_runtime::traits::Hash;
    use std::fmt::{Display, Formatter, Result as FmtResult};

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

    pub struct CreatorId(pub u64);

    impl From<u64> for CreatorId {
        fn from(id: u64) -> Self {
            CreatorId(id)
        }
    }

    #[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
    pub struct NodeMap<T>(Vec<T>);

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct ControlHash<H: Hash> {
        parents: NodeMap<bool>,
        hash: H,
    }

    pub struct CHUnit<H: Hash> {
        creator: CreatorId,
        round: u64,
        epoch_id: u64,
        hash: H,
        control_hash: ControlHash<H>,
        best_block: H,
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
