//! Error codes for the baby-liminal-extension pallet.
//!
//! Every extension function (like `store_key` or `verify`) comes with:
//! * its own success code,
//! * and a set of error codes (usually starting at the success code + 1).

#![allow(missing_docs)] // Error constants are self-descriptive.

// ---- `store_key` errors -------------------------------------------------------------------------
const BABY_LIMINAL_STORE_KEY_BASE: u32 = 10_000;
pub const BABY_LIMINAL_STORE_KEY_SUCCESS: u32 = BABY_LIMINAL_STORE_KEY_BASE;
pub const BABY_LIMINAL_STORE_KEY_TOO_LONG_KEY: u32 = BABY_LIMINAL_STORE_KEY_BASE + 1;
pub const BABY_LIMINAL_STORE_KEY_IDENTIFIER_IN_USE: u32 = BABY_LIMINAL_STORE_KEY_BASE + 2;
pub const BABY_LIMINAL_STORE_KEY_ERROR_UNKNOWN: u32 = BABY_LIMINAL_STORE_KEY_BASE + 3;

// ---- `verify` errors ----------------------------------------------------------------------------
const BABY_LIMINAL_VERIFY_BASE: u32 = 11_000;
pub const BABY_LIMINAL_VERIFY_SUCCESS: u32 = BABY_LIMINAL_VERIFY_BASE;
pub const BABY_LIMINAL_VERIFY_DESERIALIZING_PROOF_FAIL: u32 = BABY_LIMINAL_VERIFY_BASE + 1;
pub const BABY_LIMINAL_VERIFY_DESERIALIZING_INPUT_FAIL: u32 = BABY_LIMINAL_VERIFY_BASE + 2;
pub const BABY_LIMINAL_VERIFY_UNKNOWN_IDENTIFIER: u32 = BABY_LIMINAL_VERIFY_BASE + 3;
pub const BABY_LIMINAL_VERIFY_DESERIALIZING_KEY_FAIL: u32 = BABY_LIMINAL_VERIFY_BASE + 4;
pub const BABY_LIMINAL_VERIFY_VERIFICATION_FAIL: u32 = BABY_LIMINAL_VERIFY_BASE + 5;
pub const BABY_LIMINAL_VERIFY_INCORRECT_PROOF: u32 = BABY_LIMINAL_VERIFY_BASE + 6;
pub const BABY_LIMINAL_VERIFY_ERROR_UNKNOWN: u32 = BABY_LIMINAL_VERIFY_BASE + 7;
