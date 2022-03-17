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
