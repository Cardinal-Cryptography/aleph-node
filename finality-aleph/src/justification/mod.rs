use std::marker::PhantomData;

use aleph_primitives::{AuthoritySignature, BlockNumber, ALEPH_ENGINE_ID};
use codec::{Decode, Encode};
use sp_runtime::Justification;

use crate::{crypto::Signature, BlockIdentifier, IdentifierFor, SessionId};

mod compatibility;

pub use compatibility::{backwards_compatible_decode, versioned_encode, Error as DecodeError};

use crate::abft::SignatureSet;

/// A proof of block finality, currently in the form of a sufficiently long list of signatures or a
/// sudo signature of a block for emergency finalization.
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub enum AlephJustification {
    CommitteeMultisignature(SignatureSet<Signature>),
    EmergencySignature(AuthoritySignature),
}

impl From<AlephJustification> for Justification {
    fn from(val: AlephJustification) -> Self {
        (ALEPH_ENGINE_ID, versioned_encode(val))
    }
}

pub trait Verifier<BI: BlockIdentifier> {
    fn verify(&self, justification: &AlephJustification, block_id: &BI) -> bool;
}

pub struct SessionInfo<BI: BlockIdentifier, V: Verifier<BI>> {
    pub current_session: SessionId,
    pub last_block_height: BlockNumber,
    pub verifier: Option<V>,
    _phantom: PhantomData<BI>,
}

impl<BI: BlockIdentifier, V: Verifier<BI>> SessionInfo<BI, V> {
    pub fn new(
        current_session: SessionId,
        last_block_height: BlockNumber,
        verifier: Option<V>,
    ) -> Self {
        Self {
            current_session,
            last_block_height,
            verifier,
            _phantom: PhantomData,
        }
    }
}

/// Returns `SessionInfo` for the session regarding block with no. `number`.
#[async_trait::async_trait]
pub trait SessionInfoProvider<BI: BlockIdentifier, V: Verifier<BI>> {
    async fn for_block_num(&self, number: BlockNumber) -> SessionInfo<BI, V>;
}
