use crate::{Config, ValidatorsChange, ValidatorsChangeStorageItem};
use frame_support::log;
use frame_support::{
    traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
    weights::Weight,
};
use sp_std::prelude::*;

frame_support::generate_storage_alias!(
    Aleph, SessionForValidatorsChange => Value<u32>
);

frame_support::generate_storage_alias!(
    Aleph, Validators<T: Config> => Value<Option<Vec<T::AccountId>>>
);

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let new_storage_version = crate::STORAGE_VERSION;

    if on_chain_storage_version == StorageVersion::default() && new_storage_version == 1 {
        log::info!(target: "pallet_aleph", "Running migration from STORAGE_VERSION 0 to 1");

        // read the current storage values
        let session_for_validators_change = match SessionForValidatorsChange::get() {
            None => u32::default(),
            Some(session) => session,
        };

        let validators = match Validators::<T>::get() {
            None => Vec::new(),
            Some(validators) => validators,
        };

        // store under new key
        ValidatorsChange::<T>::put(ValidatorsChangeStorageItem::<T> {
            validators,
            session_for_validators_change,
        });

        // cleanup
        sp_io::storage::clear(&storage_key("Aleph", "Validators"));
        sp_io::storage::clear(&storage_key("Aleph", "SessionForValidatorsChange"));

        // store new version
        StorageVersion::new(1).put::<P>();

        // for safety take the whole block
        T::BlockWeights::get().max_block
    } else {
        log::warn!(
            target: "pallet_aleph",
            "Do not know which migration to apply because on-chain storage version is {:?} and the version declared in the aleph pallet is {:?}",
            on_chain_storage_version,
            new_storage_version
        );
        T::DbWeight::get().reads(1)
    }
}

fn storage_key(module: &str, version: &str) -> [u8; 32] {
    let pallet_name = sp_io::hashing::twox_128(module.as_bytes());
    let postfix = sp_io::hashing::twox_128(version.as_bytes());

    let mut final_key = [0u8; 32];
    final_key[..16].copy_from_slice(&pallet_name);
    final_key[16..].copy_from_slice(&postfix);

    final_key
}
