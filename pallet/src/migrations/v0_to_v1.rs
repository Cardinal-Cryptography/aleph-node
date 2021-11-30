use crate::Config;
use frame_support::log;
use frame_support::{
    traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
    weights::Weight,
};
use sp_std::prelude::*;

frame_support::generate_storage_alias!(
    Aleph, OldSessionForValidatorsChange => Value<Option<u32>>
);

frame_support::generate_storage_alias!(
    Aleph, OldValidators<T: Config> => Value<Option<Vec<T::AccountId>>>
);

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let new_storage_version = crate::STORAGE_VERSION;

    if on_chain_storage_version == StorageVersion::default() && new_storage_version == 1 {
        log::info!(target: "pallet_aleph", "Running migration from STORAGE_VERSION 0 to 1");

        let mut writes = 0;
        let _ = OldSessionForValidatorsChange::translate(|old| match old {
            Some(current) => {
                writes += 1;
                current
            }
            None => None,
        });

        let _ = OldValidators::<T>::translate(|old| match old {
            Some(current) => {
                writes += 1;
                current
            }
            None => None,
        });

        // store new version
        StorageVersion::new(1).put::<P>();
        writes += 1;

        T::DbWeight::get().reads(3) + T::DbWeight::get().writes(writes)
    } else {
        log::warn!(
            target: "pallet_aleph",
            "Do not know which migration to apply because on-chain storage version is {:?} and the version declared in the aleph pallet is {:?}",
            on_chain_storage_version,
            new_storage_version
        );
        // I have only read the version
        T::DbWeight::get().reads(1)
    }
}
