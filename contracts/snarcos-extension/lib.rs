#![cfg_attr(not(feature = "std"), no_std)]

/// Gathers all the possible errors that might occur while calling `pallet_snarcos::store_key`.
#[derive(Copy, Clone, Eq, PartialEq, Debug, scale::Decode, scale::Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum StoreKeyError {
    /// This verification key identifier is already taken.
    IdentifierAlreadyInUse,
    /// Provided verification key is longer than `pallet_snarcos::MaximumVerificationKeyLength`
    /// limit.
    VerificationKeyTooLong,
    /// Unknown status code has been returned.
    ///
    /// This is to avoid panicking from status code mismatch.
    UnknownError,
}

impl ink::env::chain_extension::FromStatusCode for StoreKeyError {
    fn from_status_code(status_code: u32) -> Result<(), Self> {
        match status_code {
            0 => Ok(()),
            1 => Err(Self::VerificationKeyTooLong),
            2 => Err(Self::IdentifierAlreadyInUse),
            _ => Err(Self::UnknownError),
        }
    }
}

#[ink::chain_extension]
pub trait FetchRandom {
    type ErrorCode = StoreKeyError;

    #[ink(extension = 41, returns_result = false)]
    fn store_key(subject: [u8; 32]);
}
