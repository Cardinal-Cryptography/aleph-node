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

pub fn timestamp_from_nanos(timestamp: u128) -> time::Time {
    time::Time::from_unix_timestamp(0, timestamp.try_into().unwrap())
        .expect("Cannot parse timestamp")
}

pub fn account_id_from_bytes(validator_address: Vec<u8>) -> account::Id {
    account::Id::try_from(validator_address).expect("Cannot create account Id")
}
