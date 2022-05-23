//! This module defines common types, in particular those that are needed for storage navigation.
//!
//! Some part of them are just wrappers for `String`. They are implemented as new unit structures.
//! This is because when `S` is a type alias for `String` then there is no way of passing `&S`
//! to a function, as `clippy` screams outrageously about changing it to `&str` and then the alias
//! is useless.

use std::{collections::HashMap, fmt::Debug, str::FromStr};

use codec::Encode;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sp_io::hashing::{blake2_128, twox_128};

pub trait Get<T = String> {
    fn get(self) -> T;
}

/// Remove leading `"0x"`.
fn strip_hex<T: ToString + ?Sized>(t: &T) -> String {
    let s = t.to_string();
    s.strip_prefix("0x").map(ToString::to_string).unwrap_or(s)
}

/// Prepend leading `"0x"`.
fn as_hex<T: ToString + ?Sized>(t: &T) -> String {
    let s = t.to_string();
    if s.starts_with("0x") {
        s
    } else {
        format!("0x{}", s)
    }
}

/// For now, we accept only 64-char-format accounts.
///
/// For `//Alice` it would be: `0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AccountId(String);

impl AccountId {
    pub fn new<T: ToString + ?Sized>(account: &T) -> Self {
        Self(as_hex(account))
    }
}

impl Get for AccountId {
    fn get(self) -> String {
        as_hex(&self.0)
    }
}

/// Human-readable, dot-separated path to storage, e.g. `System.Account`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StoragePath(String);

/// Casting from `String`, useful in parsing configuration.
impl FromStr for StoragePath {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl Get for StoragePath {
    fn get(self) -> String {
        self.0
    }
}

/// Hex-encoded key in raw chainspec.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StorageKey(String);

impl StorageKey {
    pub fn new<T: ToString + ?Sized>(content: &T) -> Self {
        Self(as_hex(content))
    }

    /// Concatenate two storage keys by appending `other` to `self`.
    pub fn join(&self, other: &StorageKey) -> StorageKey {
        let content = format!("{}{}", self.0, strip_hex(&other.0));
        StorageKey::new(&content)
    }

    /// Check whether `other` is prefix of `self`.
    pub fn is_prefix_of(&self, other: &StorageKey) -> bool {
        // We have to ensure that both items are in the same form.
        let shorter = as_hex(&self.0);
        let longer = as_hex(&other.0);
        longer.starts_with(&shorter)
    }
}

/// Copied from `frame_support`.
fn blake2_128concat(x: &[u8]) -> Vec<u8> {
    blake2_128(x).iter().chain(x.iter()).cloned().collect()
}

/// Convert `AccountId` to `StorageKey` using `Blake2_128Concat` hashing algorithm.
///
/// This is a common way of deriving storage map key for an account: see `substrate-api-client`
/// for reference.
///
/// Note however, that there may be some maps in the storage for which the (partial) key is
/// computed in other manner.
impl From<AccountId> for StorageKey {
    fn from(account: AccountId) -> StorageKey {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(strip_hex(&account.0), &mut bytes).unwrap();

        let encoded_account = bytes.encode();
        let hash = blake2_128concat(encoded_account.as_slice());
        StorageKey::new(&hash.encode_hex::<String>())
    }
}

/// Convert `StoragePath` to `StorageKey` by encoding each module separately with `twox_128`
/// algorithm and then concatenating results.
impl From<StoragePath> for StorageKey {
    fn from(path: StoragePath) -> StorageKey {
        let modules = path.0.split('.');
        let hashes = modules.flat_map(|module| twox_128(module.as_bytes()));
        StorageKey::new(&hex::encode(hashes.collect::<Vec<_>>()))
    }
}

impl Get for StorageKey {
    /// Return empty string for empty key or "0x"-prefixed key content.
    fn get(self) -> String {
        if self.0.is_empty() {
            self.0
        } else {
            as_hex(&self.0)
        }
    }
}

/// Hex-encoded value in raw chainspec.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StorageValue(String);

impl StorageValue {
    pub fn new<T: ToString + ?Sized>(value: &T) -> Self {
        Self(as_hex(value))
    }
}

impl Get for StorageValue {
    fn get(self) -> String {
        as_hex(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlockHash(String);

impl BlockHash {
    pub fn new<T: ToString + ?Sized>(hash: &T) -> Self {
        Self(as_hex(hash))
    }
}

impl Get for BlockHash {
    fn get(self) -> String {
        as_hex(&self.0)
    }
}

/// Content of `chainspec["genesis"]["raw"]["top"]`.
pub type Storage = HashMap<StorageKey, StorageValue>;

pub type Balance = u128;
