use crate::types::{TendermintVoteSignature, TimestampStorage};
use ::time::{format_description::well_known::Rfc3339, OffsetDateTime};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer};
use sp_core::{H256, H512};
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use std::{fmt::Display, str::FromStr};
use tendermint::{
    account,
    hash::{self, Hash},
};

pub fn sha256_from_bytes(bytes: &[u8]) -> Hash {
    Hash::from_bytes(hash::Algorithm::Sha256, bytes).expect("Can't produce Hash from raw bytes")
}

pub fn account_id_from_bytes(bytes: [u8; 20]) -> account::Id {
    account::Id::new(bytes)
}

/// create Vec of known size filled with empty bytes
pub fn empty_bytes(size: i32) -> Vec<u8> {
    (0..size).map(|_| u8::default()).collect()
}

pub fn as_tendermint_signature(
    signature: TendermintVoteSignature,
) -> Result<tendermint::Signature, tendermint::Error> {
    tendermint::Signature::try_from(signature.as_bytes())
}

/// Deserialize unix timestamp from rfc3339 formatted string
#[cfg(feature = "std")]
pub fn timestamp_from_rfc3339(s: &str) -> Result<TimestampStorage, &str> {
    match OffsetDateTime::parse(s, &Rfc3339) {
        Ok(datetime) => {
            let seconds = datetime.unix_timestamp();
            let nanos = datetime.nanosecond();
            Ok(TimestampStorage { seconds, nanos })
        }
        Err(_) => Err("Not in rfc3339 format"),
    }
}

/// Deserialize unix timestamp from rfc3339 formatted string
#[cfg(feature = "std")]
pub fn deserialize_timestamp_from_rfc3339<'de, D>(
    deserializer: D,
) -> Result<TimestampStorage, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    timestamp_from_rfc3339(&s).map_err(serde::de::Error::custom)
}

/// Deserialize from string if type allows it
#[cfg(feature = "std")]
pub fn deserialize_from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(serde::de::Error::custom)
}

/// Deserialize string into Vec<u8>
#[cfg(feature = "std")]
pub fn deserialize_string_as_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    Ok(string.as_bytes().to_vec())
}

/// Deserialize base64string into H512
#[cfg(feature = "std")]
pub fn base64string_as_h512(s: &str) -> Result<H512, &str> {
    match base64::decode(&s) {
        Ok(bytes) => Ok(H512::from_slice(&bytes)),
        Err(_) => Err("Not base64 encoded string"),
    }
}

/// Deserialize base64string into H256
#[cfg(feature = "std")]
pub fn deserialize_base64string_as_h256<'de, D>(deserializer: D) -> Result<H256, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let bytes = base64::decode(&string).map_err(serde::de::Error::custom)?;
    Ok(H256::from_slice(&bytes))
}

/// Maps Tendermint's hash to H256 type.
/// Since Tendermint hash type has a `None` variant and Substrate uses
/// a fixed-size array for this, function has to return `Option` to faithfully
/// represent the original value.
///
/// Returns `None` if input value is `None` variant.
pub fn tendermint_hash_to_h256(tm_hash: &Hash) -> Option<H256> {
    match tm_hash {
        Hash::Sha256(hash) => Some(H256::from_slice(hash)),
        Hash::None => None,
    }
}
