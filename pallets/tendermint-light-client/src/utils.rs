use sp_std::vec::Vec;
use tendermint::{
    account,
    hash::{self, Hash},
    time,
};

pub fn sha256_from_bytes(hash: &[u8]) -> Hash {
    // TODO type enforce 32 bytes
    Hash::from_bytes(hash::Algorithm::Sha256, hash).expect("Can't produce hash from bytes")
}

pub fn from_unix_timestamp(seconds: i64) -> time::Time {
    time::Time::from_unix_timestamp(seconds, 0).expect("Cannot parse as Time")
}

pub fn account_id_from_bytes(validator_address: Vec<u8>) -> account::Id {
    account::Id::try_from(validator_address).expect("Cannot create account Id")
}
