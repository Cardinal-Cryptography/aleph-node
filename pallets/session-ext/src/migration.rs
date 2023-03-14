use frame_support::{
    log::info,
    migration::move_storage_from_pallet,
    pallet_prelude::{Get, PalletInfoAccess, StorageVersion},
    storage::generator::StorageValue,
    traits::OnRuntimeUpgrade,
    weights::Weight,
    StoragePrefixedMap,
};
#[cfg(feature = "try-runtime")]
use {
    frame_support::{ensure, traits::STORAGE_VERSION_STORAGE_KEY_POSTFIX},
    pallets_support::ensure_storage_version,
    sp_io::hashing::twox_128,
    sp_std::vec::Vec,
};

use crate::{
    BanConfig, Banned, Config, Pallet, SessionValidatorBlockCount,
    UnderperformedValidatorSessionCount, ValidatorEraTotalReward,
};

const LOG_TARGET: &str = "pallet-session-ext";
const OLD_PREFIX: &str = "Elections";

/// migrate prefixes from Elections to this pallet.
pub struct PrefixMigration<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for PrefixMigration<T> {
    fn on_runtime_upgrade() -> Weight {
        if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(0) {
            return Weight::zero();
        };

        let pallet_name = Pallet::<T>::name();

        let prefix = SessionValidatorBlockCount::<T>::storage_prefix();
        move_storage_from_pallet(prefix, OLD_PREFIX.as_bytes(), pallet_name.as_bytes());
        info!(target: LOG_TARGET, "Migrated SessionValidatorBlockCount");

        let prefix = ValidatorEraTotalReward::<T>::storage_prefix();
        move_storage_from_pallet(prefix, OLD_PREFIX.as_bytes(), pallet_name.as_bytes());
        info!(target: LOG_TARGET, "Migrated ValidatorEraTotalReward");

        let prefix = BanConfig::<T>::storage_prefix();
        move_storage_from_pallet(prefix, OLD_PREFIX.as_bytes(), pallet_name.as_bytes());
        info!(target: LOG_TARGET, "Migrated BanConfig");

        let prefix = UnderperformedValidatorSessionCount::<T>::storage_prefix();
        move_storage_from_pallet(prefix, OLD_PREFIX.as_bytes(), pallet_name.as_bytes());
        info!(
            target: LOG_TARGET,
            "Migrated UnderperformedValidatorSessionCount"
        );

        let prefix = Banned::<T>::storage_prefix();
        move_storage_from_pallet(prefix, OLD_PREFIX.as_bytes(), pallet_name.as_bytes());
        info!(target: LOG_TARGET, "Migrated Banned");

        <T as frame_system::Config>::BlockWeights::get().max_block
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        ensure_storage_version::<Pallet<T>>(0)?;

        let pallet_name = Pallet::<T>::name();

        let pallet_prefix = twox_128(pallet_name.as_bytes());
        let storage_version_key = twox_128(STORAGE_VERSION_STORAGE_KEY_POSTFIX);

        let mut pallet_prefix_iter = frame_support::storage::KeyPrefixIterator::new(
            pallet_prefix.to_vec(),
            pallet_prefix.to_vec(),
            |key| Ok(key.to_vec()),
        );

        // Ensure nothing except the storage_version_key is stored in the new prefix.
        ensure!(
            pallet_prefix_iter.all(|key| key == storage_version_key),
            "Only storage version should be stored in the pallet"
        );

        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
        ensure_storage_version::<Pallet<T>>(0)?;

        let pallet_name = Pallet::<T>::name();

        let pallet_prefix = twox_128(pallet_name.as_bytes());

        let pallet_prefix_iter = frame_support::storage::KeyPrefixIterator::new(
            pallet_prefix.to_vec(),
            pallet_prefix.to_vec(),
            |key| Ok(key.to_vec()),
        );

        // Ensure storages hae been moved to new prefix.
        ensure!(
            pallet_prefix_iter.count() > 1,
            "No storage has been moved to this pallet prefix"
        );

        Ok(())
    }
}
