use std::time::Duration;

use aleph_bft::SignatureSet;
use aleph_primitives::AuthoritySignature;
use codec::{Decode, Encode};
use sp_api::{BlockT, NumberFor};

use crate::{crypto::Signature, SessionId};

mod compatibility;
mod handler;
mod requester;
mod scheduler;

pub use compatibility::{backwards_compatible_decode, versioned_encode, Error as DecodeError};
pub use handler::JustificationHandler;
pub use scheduler::{
    JustificationRequestScheduler, JustificationRequestSchedulerImpl, SchedulerActions,
};

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

#[derive(Clone)]
pub struct JustificationHandlerConfig<B: BlockT> {
    /// How long should we wait when the session verifier is not yet available.
    verifier_timeout: Duration,
    /// How long should we wait for any notification.
    notification_timeout: Duration,
    ///Distance (in amount of blocks) between the best and the block we want to request justification
    min_allowed_delay: NumberFor<B>,
}

impl<B: BlockT> Default for JustificationHandlerConfig<B> {
    fn default() -> Self {
        Self {
            verifier_timeout: Duration::from_millis(500),
            notification_timeout: Duration::from_millis(1000),
            min_allowed_delay: 3u32.into(),
        }
    }
}

#[cfg(test)]
impl<B: BlockT> JustificationHandlerConfig<B> {
    pub fn new(
        verifier_timeout: Duration,
        notification_timeout: Duration,
        min_allowed_delay: NumberFor<B>,
    ) -> Self {
        Self {
            verifier_timeout,
            notification_timeout,
            min_allowed_delay,
        }
    }
}
