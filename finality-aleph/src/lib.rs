#![allow(clippy::type_complexity)]
use sp_core::traits::BareCryptoStorePtr;
use temp::*;

pub(crate) mod communication;
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

/// Ties an authority identification and a cryptography keystore together for use in
/// signing that requires an authority.
pub struct AuthorityCryptoStore {
    authority_id: AuthorityId,
    crypto_store: BareCryptoStorePtr,
}

impl AuthorityCryptoStore {
    /// Constructs a new authority cryptography keystore.
    pub fn new(authority_id: AuthorityId, crypto_store: BareCryptoStorePtr) -> Self {
        AuthorityCryptoStore {
            authority_id,
            crypto_store,
        }
    }

    /// Returns a references to the authority id.
    pub fn authority_id(&self) -> &AuthorityId {
        &self.authority_id
    }

    /// Returns a reference to the cryptography keystore.
    pub fn crypto_store(&self) -> &BareCryptoStorePtr {
        &self.crypto_store
    }
}

impl AsRef<BareCryptoStorePtr> for AuthorityCryptoStore {
    fn as_ref(&self) -> &BareCryptoStorePtr {
        self.crypto_store()
    }
}

/// Temporary structs and traits until initial version of Aleph is published.
pub(crate) mod temp {
    use codec::{Decode, Encode};
    use sp_runtime::traits::Block;
    use std::fmt::{Display, Formatter, Result as FmtResult};

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
    pub struct NodeIndex(pub(crate) u32);

    impl Display for NodeIndex {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{}", self.0)
        }
    }

    impl From<u32> for NodeIndex {
        fn from(idx: u32) -> Self {
            NodeIndex(idx)
        }
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
    pub struct Round(pub u32);

    impl Display for Round {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{}", self.0)
        }
    }

    impl From<u32> for Round {
        fn from(id: u32) -> Self {
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

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
    pub struct CreatorId(pub u32);

    impl From<u32> for CreatorId {
        fn from(id: u32) -> Self {
            CreatorId(id)
        }
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
    pub struct UnitCoord {
        pub creator: CreatorId,
        pub round: Round,
    }

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

    #[derive(Debug, Encode, Decode, Clone)]
    pub struct Unit<B: Block> {
        pub creator: CreatorId,
        pub round: Round,
        pub epoch_id: EpochId,
        pub hash: <B as Block>::Hash,
        pub control_hash: ControlHash<<B as Block>::Hash>,
        pub best_block: <B as Block>::Hash,
    }

    impl<B: Block> From<Unit<B>> for UnitCoord {
        fn from(unit: Unit<B>) -> Self {
            UnitCoord {
                creator: unit.creator,
                round: unit.round,
            }
        }
    }

    impl<B: Block> From<&Unit<B>> for UnitCoord {
        fn from(unit: &Unit<B>) -> Self {
            UnitCoord {
                creator: unit.creator,
                round: unit.round,
            }
        }
    }
}

use temp::*;
