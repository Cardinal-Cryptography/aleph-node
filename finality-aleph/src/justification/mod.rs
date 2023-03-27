use aleph_primitives::AuthoritySignature;
use codec::{Decode, Encode};
use sp_api::{BlockT, NumberFor};

use crate::{crypto::Signature, SessionId};

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

pub trait Verifier<B: BlockT> {
    fn verify(&self, justification: &AlephJustification, hash: B::Hash) -> bool;
}

pub struct SessionInfo<B: BlockT, V: Verifier<B>> {
    pub current_session: SessionId,
    pub last_block_height: NumberFor<B>,
    pub verifier: Option<V>,
}

/// Returns `SessionInfo` for the session regarding block with no. `number`.
#[async_trait::async_trait]
pub trait SessionInfoProvider<B: BlockT, V: Verifier<B>> {
    async fn for_block_num(&self, number: NumberFor<B>) -> SessionInfo<B, V>;
}

/// A notification for sending justifications over the network.
#[derive(Clone)]
pub struct JustificationNotification<Block: BlockT> {
    /// The justification itself.
    pub justification: AlephJustification,
    /// The hash of the finalized block.
    pub hash: Block::Hash,
    /// The ID of the finalized block.
    pub number: NumberFor<Block>,
}
