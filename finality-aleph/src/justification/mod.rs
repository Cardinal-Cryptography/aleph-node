use std::{fmt::Display, marker::PhantomData, time::Duration};

use aleph_primitives::{AuthoritySignature, BlockNumber, ALEPH_ENGINE_ID};
use codec::{Decode, Encode};

use crate::{crypto::Signature, BlockIdentifier, IdentifierFor};

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

pub trait Verifier<BI: BlockIdentifier> {
    fn verify(&self, justification: &AlephJustification, block_id: &BI) -> bool;
}

pub struct SessionInfo<BI: BlockIdentifier, V: Verifier<BI>> {
    pub last_block_height: BlockNumber,
    pub verifier: V,
    _phantom: PhantomData<BI>,
}

impl<BI: BlockIdentifier, V: Verifier<BI>> SessionInfo<BI, V> {
    pub fn new(last_block_height: BlockNumber, verifier: V) -> Self {
        Self {
            last_block_height,
            verifier,
            _phantom: PhantomData,
        }
    }
}

/// Returns `SessionInfo` for the session regarding block with no. `number`.
#[async_trait::async_trait]
pub trait SessionInfoProvider<BI: BlockIdentifier, V: Verifier<BI>> {
    type Error: Display;

    async fn for_block_num(&self, number: BlockNumber) -> Result<SessionInfo<BI, V>, Self::Error>;
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
    /// How long should we wait for any notification.
    notification_timeout: Duration,
}

impl Default for JustificationHandlerConfig {
    fn default() -> Self {
        Self {
            // request justifications slightly more frequently than they're created
            notification_timeout: Duration::from_millis(800),
        }
    }
}

#[cfg(test)]
impl JustificationHandlerConfig {
    pub fn new(notification_timeout: Duration) -> Self {
        Self {
            notification_timeout,
        }
    }
}
