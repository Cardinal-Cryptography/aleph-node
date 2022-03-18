#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer};
use sp_core::{H256, H512};
use sp_std::vec::Vec;
use tendermint::{
    account,
    hash::{self, Hash},
    time,
};

pub fn sha256_from_bytes(bytes: &[u8]) -> Hash {
    Hash::from_bytes(hash::Algorithm::Sha256, bytes).expect("Can't produce Hash from raw bytes")
}

pub fn from_unix_timestamp(seconds: i64) -> time::Time {
    time::Time::from_unix_timestamp(seconds, 0).expect("Cannot parse as Time")
}

pub fn account_id_from_bytes(bytes: [u8; 20]) -> account::Id {
    account::Id::new(bytes)
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
pub fn deserialize_base64string_as_h512<'de, D>(deserializer: D) -> Result<H512, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let bytes = base64::decode(&string).map_err(serde::de::Error::custom)?;
    Ok(H512::from_slice(&bytes))
}

#[cfg(feature = "std")]
pub fn deserialize_base64string_as_h256<'de, D>(deserializer: D) -> Result<H256, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let bytes = base64::decode(&string).map_err(serde::de::Error::custom)?;
    Ok(H256::from_slice(&bytes))
}
