//! # Baby Liminal Extension
//!
//! This crate provides a way for smart contracts to interact with the [`pallet_baby_liminal`]
//! runtime module.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

// Rust features are additive, so this is the only way we can ensure that only one of these is
// enabled.
#[cfg(all(feature = "ink", feature = "runtime"))]
compile_error!("Features `ink` and `runtime` are mutually exclusive and cannot be used together");

// ------ Common stuff -----------------------------------------------------------------------------

pub mod extension_ids;
pub mod status_codes;

/// Copied from `pallet_baby_liminal`.
pub type VerificationKeyIdentifier = [u8; 8];

// ------ Frontend stuff ---------------------------------------------------------------------------

#[cfg(feature = "ink")]
mod frontend;

#[cfg(feature = "ink")]
pub use frontend::{BabyLiminalError, BabyLiminalExtension, Environment};

// ------ Backend stuff ----------------------------------------------------------------------------

#[cfg(feature = "runtime")]
mod backend;

#[cfg(feature = "runtime")]
pub use backend::BabyLiminalChainExtension;
