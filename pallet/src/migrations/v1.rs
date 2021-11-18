use crate::Config;
use frame_support::log;

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
    log::info!(
        target: "pallet_aleph",
        "Running migration to v1 for pallet_aleph with storage version {:?}",
        on_chain_storage_version,
    );

    0
}
