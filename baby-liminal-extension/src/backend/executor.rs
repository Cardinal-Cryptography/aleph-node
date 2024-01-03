use pallet_contracts::Config as ContractsConfig;
use pallet_vk_storage::Config as BabyLiminalConfig;
use primitives::liminal::VerifierError;

use crate::args::VerifyArgs;

/// Represents an 'engine' that handles chain extension calls.
pub trait BackendExecutor {
    fn verify(args: VerifyArgs) -> Result<(), VerifierError>;
}

/// Minimal runtime configuration required by the standard chain extension executor.
pub trait MinimalRuntime: BabyLiminalConfig + ContractsConfig {}
impl<R: BabyLiminalConfig + ContractsConfig> MinimalRuntime for R {}

/// Default implementation for the chain extension mechanics.
impl<Runtime: MinimalRuntime> BackendExecutor for Runtime {
    fn verify(_args: VerifyArgs) -> Result<(), VerifierError> {
        primitives::liminal::snark_verifier::verify()
    }
}
