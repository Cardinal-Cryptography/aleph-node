//! This is the frontend of the chain extension, i.e., the part exposed to the smart contracts.

use ink::{
    env::{DefaultEnvironment, Environment as EnvironmentT},
    prelude::vec::Vec,
    primitives::AccountId,
};

use crate::VerificationKeyIdentifier;

#[derive(Debug, Copy, Clone, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[allow(missing_docs)] // Error variants are self-descriptive.
/// Chain extension errors enumeration.
pub enum BabyLiminalError {
    // `pallet_baby_liminal::store_key` errors
    VerificationKeyTooLong,
    IdentifierAlreadyInUse,
    StoreKeyErrorUnknown,

    // `pallet_baby_liminal::verify` errors
    UnknownVerificationKeyIdentifier,
    DeserializingProofFailed,
    DeserializingPublicInputFailed,
    DeserializingVerificationKeyFailed,
    VerificationFailed,
    IncorrectProof,
    VerifyErrorUnknown,

    /// Couldn't serialize or deserialize data.
    ScaleError,
    /// Unexpected error code has been returned.
    UnknownError(u32),
}

impl From<scale::Error> for BabyLiminalError {
    fn from(_: scale::Error) -> Self {
        Self::ScaleError
    }
}

impl ink::env::chain_extension::FromStatusCode for BabyLiminalError {
    fn from_status_code(status_code: u32) -> Result<(), Self> {
        use crate::error_codes::*;

        match status_code {
            // Success codes
            BABY_LIMINAL_STORE_KEY_SUCCESS | BABY_LIMINAL_VERIFY_SUCCESS => Ok(()),

            // `pallet_baby_liminal::store_key` errors
            BABY_LIMINAL_STORE_KEY_TOO_LONG_KEY => Err(Self::VerificationKeyTooLong),
            BABY_LIMINAL_STORE_KEY_IDENTIFIER_IN_USE => Err(Self::IdentifierAlreadyInUse),
            BABY_LIMINAL_STORE_KEY_ERROR_UNKNOWN => Err(Self::StoreKeyErrorUnknown),

            // `pallet_baby_liminal::verify` errors
            BABY_LIMINAL_VERIFY_DESERIALIZING_PROOF_FAIL => Err(Self::DeserializingProofFailed),
            BABY_LIMINAL_VERIFY_DESERIALIZING_INPUT_FAIL => {
                Err(Self::DeserializingPublicInputFailed)
            }
            BABY_LIMINAL_VERIFY_UNKNOWN_IDENTIFIER => Err(Self::UnknownVerificationKeyIdentifier),
            BABY_LIMINAL_VERIFY_DESERIALIZING_KEY_FAIL => {
                Err(Self::DeserializingVerificationKeyFailed)
            }
            BABY_LIMINAL_VERIFY_VERIFICATION_FAIL => Err(Self::VerificationFailed),
            BABY_LIMINAL_VERIFY_INCORRECT_PROOF => Err(Self::IncorrectProof),
            BABY_LIMINAL_VERIFY_ERROR_UNKNOWN => Err(Self::VerifyErrorUnknown),

            unexpected => Err(Self::UnknownError(unexpected)),
        }
    }
}

/// BabyLiminal chain extension definition.
#[ink::chain_extension]
pub trait BabyLiminalExtension {
    type ErrorCode = BabyLiminalError;

    /// Directly call `pallet_baby_liminal::store_key`.
    #[ink(extension = 41)]
    fn store_key(
        origin: AccountId,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
    ) -> Result<(), BabyLiminalError>;

    /// Directly call `pallet_baby_liminal::verify`.
    #[ink(extension = 42)]
    fn verify(
        identifier: VerificationKeyIdentifier,
        proof: Vec<u8>,
        input: Vec<u8>,
    ) -> Result<(), BabyLiminalError>;
}

/// Default ink environment with `BabyLiminalExtension` included.
#[derive(Debug, Copy, Clone, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Environment {}

impl EnvironmentT for Environment {
    const MAX_EVENT_TOPICS: usize = <DefaultEnvironment as EnvironmentT>::MAX_EVENT_TOPICS;

    type AccountId = <DefaultEnvironment as EnvironmentT>::AccountId;
    type Balance = <DefaultEnvironment as EnvironmentT>::Balance;
    type Hash = <DefaultEnvironment as EnvironmentT>::Hash;
    type BlockNumber = <DefaultEnvironment as EnvironmentT>::BlockNumber;
    type Timestamp = <DefaultEnvironment as EnvironmentT>::Timestamp;

    type ChainExtension = BabyLiminalExtension;
}
