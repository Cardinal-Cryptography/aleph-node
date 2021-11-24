use crate::Config;
use frame_support::log;
use frame_support::{
    traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
    weights::Weight,
};

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let new_storage_version = crate::STORAGE_VERSION;

    if on_chain_storage_version == StorageVersion::default() && new_storage_version == 1 {
        log::info!(target: "pallet_aleph", "Running migration from STORAGE_VERSION 0 to 1");

        let _ = crate::SessionForValidatorsChange::<T>::translate(|old| match old {
            Some(current) => current,
            None => None,
        });

        let _ = crate::Validators::<T>::translate(|old| match old {
            Some(current) => current,
            None => None,
        });

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
