#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(feature = "ink", feature = "substrate"))]
compile_error!(
    "Features `ink` and `substrate` are mutually exclusive and cannot be enabled together"
);

#[cfg(feature = "ink")]
pub mod ink;

#[cfg(feature = "substrate")]
pub mod substrate;

#[cfg(feature = "substrate")]
pub mod executor;

#[cfg(feature = "ink")]
use ::ink::{prelude::vec::Vec, primitives::AccountId as AccountId32};
#[cfg(feature = "substrate")]
use obce::substrate::{sp_runtime::AccountId32, sp_std::vec::Vec};

// `pallet_baby_liminal::store_key_pair` errors
const BABY_LIMINAL_STORE_KEY_PAIR_ERROR: u32 = 10_000;
pub const BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_KEY_PAIR: u32 =
    BABY_LIMINAL_STORE_KEY_PAIR_ERROR + 1;
pub const BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_PROVING_KEY: u32 =
    BABY_LIMINAL_STORE_KEY_PAIR_ERROR + 2;
pub const BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_VERIFICATION_KEY: u32 =
    BABY_LIMINAL_STORE_KEY_PAIR_ERROR + 3;
pub const BABY_LIMINAL_STORE_KEY_PAIR_IDENTIFIER_IN_USE: u32 =
    BABY_LIMINAL_STORE_KEY_PAIR_ERROR + 4;
pub const BABY_LIMINAL_STORE_KEY_PAIR_ERROR_UNKNOWN: u32 = BABY_LIMINAL_STORE_KEY_PAIR_ERROR + 5;

// `pallet_baby_liminal::verify` errors
const BABY_LIMINAL_VERIFY_ERROR: u32 = 11_000;
pub const BABY_LIMINAL_VERIFY_DESERIALIZING_PROOF_FAIL: u32 = BABY_LIMINAL_VERIFY_ERROR + 1;
pub const BABY_LIMINAL_VERIFY_DESERIALIZING_INPUT_FAIL: u32 = BABY_LIMINAL_VERIFY_ERROR + 2;
pub const BABY_LIMINAL_KEY_PAIR_UNKNOWN_IDENTIFIER: u32 = BABY_LIMINAL_VERIFY_ERROR + 3;
pub const BABY_LIMINAL_VERIFY_DESERIALIZING_KEY_FAIL: u32 = BABY_LIMINAL_VERIFY_ERROR + 4;
pub const BABY_LIMINAL_VERIFY_VERIFICATION_FAIL: u32 = BABY_LIMINAL_VERIFY_ERROR + 5;
pub const BABY_LIMINAL_VERIFY_INCORRECT_PROOF: u32 = BABY_LIMINAL_VERIFY_ERROR + 6;
pub const BABY_LIMINAL_VERIFY_ERROR_UNKNOWN: u32 = BABY_LIMINAL_VERIFY_ERROR + 7;

/// Chain extension errors enumeration.
///
/// All inner variants are convertible to [`RetVal`]
/// via [`TryFrom`] impl.
///
/// To ensure that [`RetVal`] is returned when possible in implementation
/// its methods should be marked with `#[obce(ret_val)]`.
///
/// [`RetVal`]: obce::substrate::pallet_contracts::chain_extension::RetVal
#[obce::error]
pub enum BabyLiminalError {
    // `pallet_baby_liminal::store_key_pair` errors
    #[obce(ret_val = "BABY_LIMINAL_STORE_KEY_PAIR_IDENTIFIER_IN_USE")]
    IdentifierAlreadyInUse,
    #[obce(ret_val = "BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_PROVING_KEY")]
    ProvingKeyTooLong,
    #[obce(ret_val = "BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_VERIFICATION_KEY")]
    VerificationKeyTooLong,
    #[obce(ret_val = "BABY_LIMINAL_STORE_KEY_PAIR_ERROR_UNKNOWN")]
    StoreKeyPairErrorUnknown,

    // `pallet_baby_liminal::verify` errors
    #[obce(ret_val = "BABY_LIMINAL_KEY_PAIR_UNKNOWN_IDENTIFIER")]
    UnknownKeyPairIdentifier,
    #[obce(ret_val = "BABY_LIMINAL_VERIFY_DESERIALIZING_PROOF_FAIL")]
    DeserializingProofFailed,
    #[obce(ret_val = "BABY_LIMINAL_VERIFY_DESERIALIZING_INPUT_FAIL")]
    DeserializingPublicInputFailed,
    #[obce(ret_val = "BABY_LIMINAL_VERIFY_DESERIALIZING_KEY_FAIL")]
    DeserializingVerificationKeyFailed,
    #[obce(ret_val = "BABY_LIMINAL_VERIFY_VERIFICATION_FAIL")]
    VerificationFailed,
    #[obce(ret_val = "BABY_LIMINAL_VERIFY_INCORRECT_PROOF")]
    IncorrectProof,
    #[obce(ret_val = "BABY_LIMINAL_VERIFY_ERROR_UNKNOWN")]
    VerifyErrorUnknown,
}

/// Copied from `pallet_baby_liminal`.
pub type KeyPairIdentifier = [u8; 8];

pub type SingleHashInput = (u64, u64, u64, u64);

/// BabyLiminal chain extension definition.
#[obce::definition(id = "baby-liminal-extension@v0.1")]
pub trait BabyLiminalExtension {
    /// Directly call `pallet_baby_liminal::store_key_pair`.
    #[obce(id = 41)]
    fn store_key_pair(
        &mut self,
        origin: AccountId32,
        identifier: KeyPairIdentifier,
        proving_key: Vec<u8>,
        verification_key: Vec<u8>,
    ) -> Result<(), BabyLiminalError>;

    /// Directly call `pallet_baby_liminal::verify`.
    #[obce(id = 42)]
    fn verify(
        &mut self,
        identifier: KeyPairIdentifier,
        proof: Vec<u8>,
        input: Vec<u8>,
    ) -> Result<(), BabyLiminalError>;

    #[obce(id = 43)]
    fn poseidon_one_to_one(&self, input: [SingleHashInput; 1]) -> SingleHashInput;

    #[obce(id = 44)]
    fn poseidon_two_to_one(&self, input: [SingleHashInput; 2]) -> SingleHashInput;

    #[obce(id = 45)]
    fn poseidon_four_to_one(&self, input: [SingleHashInput; 4]) -> SingleHashInput;
}
