use crate::Config;
use frame_support::log;
use frame_support::{
    traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
    weights::Weight,
};

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let current_storage_version = <P as GetStorageVersion>::current_storage_version();

    if on_chain_storage_version == 1 && current_storage_version == 2 {
        log::info!(target: "pallet_aleph", "Running migration from STORAGE_VERSION 1 to 2");

        let mut writes = 0;

        // store new version
        StorageVersion::new(2).put::<P>();
        writes += 1;

        T::DbWeight::get().reads(3) + T::DbWeight::get().writes(writes)
    } else {
        log::warn!(
            target: "pallet_aleph",
            "Not applying any storage migration because on-chain storage version is {:?} and the version declared in the aleph pallet is {:?}",
            on_chain_storage_version,
            current_storage_version
        );
        // I have only read the version
        T::DbWeight::get().reads(1)
    }
}
