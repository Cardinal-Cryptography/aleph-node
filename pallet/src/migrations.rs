use crate::Config;
use frame_support::log;
use sp_std::vec;

use frame_support::{
    storage::{generator::StorageValue, StoragePrefixedMap},
    traits::{
        Get, GetStorageVersion, PalletInfoAccess, StorageVersion,
        STORAGE_VERSION_STORAGE_KEY_POSTFIX,
    },
    weights::Weight,
};

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let new_storage_version = crate::STORAGE_VERSION;

    if on_chain_storage_version == 0 && new_storage_version == 1 {
        log::info!(
        target: "pallet_aleph",
        "Running migration from STORAGE_VERSION 0 to 1",
        );

        StorageVersion::new(1).put::<P>();

        // TODO calculate and return migration weights
        // T::DbWeight::get().reads_writes(count as Weight + 1, count as Weight + 1)
        0
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
