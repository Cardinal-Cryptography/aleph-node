#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(feature = "ink", feature = "runtime"))]
compile_error!(
    "Features `ink` and `runtime` are mutually exclusive and cannot be enabled together"
);

#[cfg(feature = "ink")]
pub mod api;
pub mod error_codes;
pub mod extension_ids;

#[cfg(feature = "ink")]
pub use api::{BabyLiminalError, BabyLiminalExtension, Environment};

/// Copied from `pallet_baby_liminal`.
pub type VerificationKeyIdentifier = [u8; 8];
