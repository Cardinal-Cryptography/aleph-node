use scale_info::{
    scale::{Decode, Encode},
    TypeInfo,
};
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;
use strum::IntoStaticStr;

/// Gathers all the possible errors that might occur while calling `pallet_snarcos::store_key` or
/// `pallet_snarcos::verify`.
///
/// Errors can be represented as `String`s.
#[derive(IntoStaticStr, Copy, Clone, Eq, PartialEq, Debug, Decode, Encode)]
#[cfg_attr(feature = "std", derive(TypeInfo))]
pub enum SnarcosError<T> {
    /// This verification key identifier is already taken.
    IdentifierAlreadyInUse,
    /// There is no verification key available under this identifier.
    UnknownVerificationKeyIdentifier,
    /// Provided verification key is longer than `MaximumVerificationKeyLength` limit.
    VerificationKeyTooLong,
    /// Couldn't deserialize proof.
    DeserializingProofFailed,
    /// Couldn't deserialize public input.
    DeserializingPublicInputFailed,
    /// Couldn't deserialize verification key from storage.
    DeserializingVerificationKeyFailed,
    /// Verification procedure has failed. Proof still can be correct.
    VerificationFailed,
    /// Proof has been found to be incorrect.
    IncorrectProof,

    /// Unknown status code has been returned.
    ///
    /// This is to avoid panicking from status code mismatch.
    UnknownError,

    Phantom(PhantomData<T>),
}

impl<T> From<SnarcosError<T>> for DispatchError {
    fn from(err: SnarcosError<T>) -> Self {
        Self::Other(err.into())
    }
}

/// We store verification keys under short identifiers.
pub type VerificationKeyIdentifier = [u8; 4];

/// Handled proving systems.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Decode, Encode, TypeInfo)]
pub enum ProvingSystem {
    Groth16,
    Gm17,
    Marlin,
}
