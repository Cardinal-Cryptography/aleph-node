use crate::{AccountId, StorageKey, StoragePath};
use codec::Encode;
use hex::ToHex;
use sp_io::hashing::{blake2_128, twox_128};

pub fn hash_storage_prefix(storage_path: &StoragePath) -> StorageKey {
    let modules = storage_path.0.split('.');
    let hashes = modules.flat_map(|module| twox_128(module.as_bytes()));
    format!("0x{}", hex::encode(hashes.collect::<Vec<_>>()))
}

fn blake2_128concat(x: &[u8]) -> Vec<u8> {
    blake2_128(x).iter().chain(x.into_iter()).cloned().collect()
}

pub fn hash_account(account: &AccountId) -> StorageKey {
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(account.0.clone(), &mut bytes).unwrap();
    let encoded_account = bytes.encode();
    let hash = blake2_128concat(encoded_account.as_slice());
    format!("0x{}", hash.encode_hex::<String>())
}

pub fn is_prefix_of(shorter: &str, longer: &str) -> bool {
    longer.starts_with(shorter)
}

pub fn combine_storage_keys(prefix: &StorageKey, suffix: &StorageKey) -> StorageKey {
    format!(
        "{}{}",
        prefix,
        suffix.strip_prefix("0x").unwrap_or(suffix.as_str())
    )
}
