//! This module defines common types, in particular those that are needed for storage navigation.
//!
//! Some part of them are just wrappers for `String`. They are implemented as new unit structures.
//! This is because when `S` is a type alias for `String` then there is no way of passing `&S`
//! to a function, as `clippy` screams outrageously about changing it to `&str` and then the alias
//! is useless.

use std::{collections::HashMap, str::FromStr};

use codec::Encode;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sp_io::hashing::{blake2_128, twox_128};

/// Hex-encoded key in raw chainspec.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StorageKey(pub String);

impl StorageKey {
    /// Concatenate two storage keys by appending `other` to `self`.
    pub fn join(&self, other: &StorageKey) -> StorageKey {
        let suffix = other.0.strip_prefix("0x").unwrap_or(other.0.as_str());
        let content = format!("{}{}", self.0, suffix);
        StorageKey(content)
    }

    /// Check whether `other` is prefix of `self`.
    pub fn is_prefix_of(&self, other: &StorageKey) -> bool {
        self.0.starts_with(&other.0)
    }
}

/// Hex-encoded value in raw chainspec.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StorageValue(pub String);

/// Human-readable, dot-separated path to storage, e.g. `System.Account`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StoragePath(pub String);

impl Into<StorageKey> for StoragePath {
    fn into(self) -> StorageKey {
        let modules = self.0.split('.');
        let hashes = modules.flat_map(|module| twox_128(module.as_bytes()));
        let content = format!("0x{}", hex::encode(hashes.collect::<Vec<_>>()));
        StorageKey(content)
    }
}

impl FromStr for StoragePath {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlockHash(pub String);

/// Content of `chainspec["genesis"]["raw"]["top"]`.
pub type Storage = HashMap<StorageKey, StorageValue>;

pub type Balance = u128;

/// For now, we accept only 64-char-format accounts.
///
/// For `//Alice` it would be: `0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);

/// Copied from `frame_support`.
fn blake2_128concat(x: &[u8]) -> Vec<u8> {
    blake2_128(x).iter().chain(x.into_iter()).cloned().collect()
}

impl Into<StorageKey> for AccountId {
    fn into(self) -> StorageKey {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(self.0, &mut bytes).unwrap();

        let encoded_account = bytes.encode();
        let hash = blake2_128concat(encoded_account.as_slice());
        StorageKey(format!("0x{}", hash.encode_hex::<String>()))
    }
}
