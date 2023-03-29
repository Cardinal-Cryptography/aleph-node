use std::time::Duration;

use aleph_primitives::{AuthoritySignature, BlockNumber, ALEPH_ENGINE_ID};
use codec::{Decode, Encode};
use sp_api::{BlockT, NumberFor};

use crate::{crypto::Signature, BlockIdentifier, IdentifierFor, SessionId};

mod compatibility;
mod handler;
mod requester;
mod scheduler;

pub use compatibility::{backwards_compatible_decode, versioned_encode, Error as DecodeError};
pub use handler::JustificationHandler;
pub use scheduler::{
    JustificationRequestScheduler, JustificationRequestSchedulerImpl, SchedulerActions,
};
use sp_runtime::Justification;

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
pub struct JustificationNotification<BI: BlockIdentifier> {
    /// The justification itself.
    pub justification: AlephJustification,
    /// The ID of the finalized block.
    pub block_id: BI,
}

pub type JustificationNotificationFor<B> = JustificationNotification<IdentifierFor<B>>;

#[derive(Clone)]
pub struct JustificationHandlerConfig {
    /// How long should we wait when the session verifier is not yet available.
    verifier_timeout: Duration,
    /// How long should we wait for any notification.
    notification_timeout: Duration,
}

impl Default for JustificationHandlerConfig {
    fn default() -> Self {
        Self {
            verifier_timeout: Duration::from_millis(500),
            // request justifications slightly more frequently than they're created
            notification_timeout: Duration::from_millis(800),
        }
    }
}

#[cfg(test)]
impl JustificationHandlerConfig {
    pub fn new(verifier_timeout: Duration, notification_timeout: Duration) -> Self {
        Self {
            verifier_timeout,
            notification_timeout,
        }
    }
}
