use pallet_contracts::Config as ContractsConfig;
use pallet_vk_storage::Config as BabyLiminalConfig;
use scale::{Decode, Encode};

use crate::args::VerifyArgs;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Encode, Decode)]
pub enum ExecutorError {
    /// No verification key available under this identifier.
    UnknownVerificationKeyIdentifier,
    /// Couldn't deserialize proof.
    DeserializingProofFailed,
    /// Couldn't deserialize public input.
    DeserializingPublicInputFailed,
    /// Couldn't deserialize verification key from storage.
    DeserializingVerificationKeyFailed,
    /// Verification procedure has failed. Proof still can be correct.
    VerificationFailed,
    /// Proof has been found as incorrect.
    IncorrectProof,
}

/// Represents an 'engine' that handles chain extension calls.
pub trait BackendExecutor {
    fn verify(args: VerifyArgs) -> Result<(), ExecutorError>;
}

/// Minimal runtime configuration required by the standard chain extension executor.
pub trait MinimalRuntime: BabyLiminalConfig + ContractsConfig {}
impl<R: BabyLiminalConfig + ContractsConfig> MinimalRuntime for R {}

/// Default implementation for the chain extension mechanics.
impl<Runtime: MinimalRuntime> BackendExecutor for Runtime {
    fn verify(_args: VerifyArgs) -> Result<(), ExecutorError> {
        Ok(())
    }
}
