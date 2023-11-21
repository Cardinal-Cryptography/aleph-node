use frame_support::{pallet_prelude::Weight, sp_runtime::AccountId32};
use frame_system::Config as SystemConfig;
use pallet_baby_liminal::{
    Config as PalletConfig, Error as PalletError, Pallet, VerificationKeyIdentifier,
};
use pallet_contracts::Config as ContractsConfig;
use sp_std::vec::Vec;

/// Generalized pallet executor, that can be mocked for testing purposes.
pub trait BackendExecutor {
    /// The pallet's error enum is generic. For most purposes however, it doesn't matter what type
    /// will be passed there. Normally, `Runtime` will be the generic argument, but in the testing
    /// context it will be enough to instantiate it with `()`.
    type ErrorGenericType;

    fn store_key(
        depositor: AccountId32,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
    ) -> Result<(), PalletError<Self::ErrorGenericType>>;

    fn verify(
        verification_key_identifier: VerificationKeyIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
    ) -> Result<(), (PalletError<Self::ErrorGenericType>, Option<Weight>)>;
}

/// Default implementation for the chain extension mechanics.
impl<Runtime: SystemConfig + PalletConfig + ContractsConfig> BackendExecutor for Runtime
where
    <Runtime as SystemConfig>::RuntimeOrigin: From<Option<AccountId32>>,
{
    type ErrorGenericType = Runtime;

    fn store_key(
        depositor: AccountId32,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
    ) -> Result<(), PalletError<Self::ErrorGenericType>> {
        Pallet::<Runtime>::bare_store_key(Some(depositor).into(), identifier, key)
    }

    fn verify(
        verification_key_identifier: VerificationKeyIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
    ) -> Result<(), (PalletError<Self::ErrorGenericType>, Option<Weight>)> {
        Pallet::<Runtime>::bare_verify(verification_key_identifier, proof, public_input)
    }
}
