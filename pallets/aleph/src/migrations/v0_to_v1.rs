use frame_support::{
    log, storage_alias,
    traits::{Get, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
    weights::Weight,
};
#[cfg(feature = "try-runtime")]
use pallets_support::ensure_storage_version;
use pallets_support::StorageMigration;
use sp_std::vec::Vec;

use crate::Config;

#[storage_alias]
type SessionForValidatorsChange = StorageValue<Aleph, u32>;

#[storage_alias]
type Validators<T> = StorageValue<Aleph, Vec<<T as frame_system::Config>::AccountId>>;

/// Flattening double `Option<>` storage.
pub struct Migration<T, P>(sp_std::marker::PhantomData<(T, P)>);

impl<T: Config, P: PalletInfoAccess> StorageMigration for Migration<T, P> {
    #[cfg(feature = "try-runtime")]
    const MIGRATION_STORAGE_PREFIX: &'static [u8] = b"PALLET_ALEPH::V0_TO_V1_MIGRATION";
}

impl<T: Config, P: PalletInfoAccess> OnRuntimeUpgrade for Migration<T, P> {
    fn on_runtime_upgrade() -> Weight {
        log::info!(target: "pallet_aleph", "Running migration from STORAGE_VERSION 0 to 1");

        let mut writes = 0;

        match SessionForValidatorsChange::translate(|old: Option<Option<u32>>| -> Option<u32> {
            log::info!(target: "pallet_aleph", "Current storage value for SessionForValidatorsChange {:?}", old);
            match old {
                Some(Some(x)) => Some(x),
                _ => None,
            }
        }) {
            Ok(_) => {
                writes += 1;
                log::info!(target: "pallet_aleph", "Successfully migrated storage for SessionForValidatorsChange");
            }
            Err(why) => {
                log::error!(target: "pallet_aleph", "Something went wrong during the migration of SessionForValidatorsChange {:?}", why);
            }
        };

        match Validators::<T>::translate(
            |old: Option<Option<Vec<T::AccountId>>>| -> Option<Vec<T::AccountId>> {
                log::info!(target: "pallet_aleph", "Current storage value for Validators {:?}", old);
                match old {
                    Some(Some(x)) => Some(x),
                    _ => None,
                }
            },
        ) {
            Ok(_) => {
                writes += 1;
                log::info!(target: "pallet_aleph", "Successfully migrated storage for Validators");
            }
            Err(why) => {
                log::error!(target: "pallet_aleph", "Something went wrong during the migration of Validators storage {:?}", why);
            }
        };

        // store new version
        StorageVersion::new(1).put::<P>();
        writes += 1;

        T::DbWeight::get().reads(2) + T::DbWeight::get().writes(writes)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        Ok(())
    }
}
