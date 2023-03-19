use frame_support::{
    log,
    pallet_prelude::*,
    traits::{Get, OnRuntimeUpgrade},
    weights::Weight,
};
use pallet_contracts::{Config, Pallet};
use sp_std::marker::PhantomData;

const TARGET: &str = "runtime::custom_contract_migration";

pub struct ContractsSetVersion9<T: Config>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for ContractsSetVersion9<T> {
    fn on_runtime_upgrade() -> Weight {
        let version = StorageVersion::get::<Pallet<T>>();
        let mut weight = T::DbWeight::get().reads_writes(1, 0);
        log::info!(
            target: TARGET,
            "On-chain version of pallet contracts is {:?}",
            version
        );
        if version < 9 {
            weight += T::DbWeight::get().reads_writes(0, 1);
            StorageVersion::new(9).put::<Pallet<T>>();
        }

        weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        let version = StorageVersion::get::<Pallet<T>>();
        log::warn!(
            target: TARGET,
            "Pre-upgrade version in custom contracts migration: {:?}",
            version
        );
        if version != 0 {
            return Err("Version should be 0, because pallet contracts is not present.");
        }

        Ok(vec![])
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
        let version = StorageVersion::get::<Pallet<T>>();
        log::warn!(
            target: TARGET,
            "Post-upgrade version in custom contracts migration: {:?}",
            version
        );
        if version != StorageVersion::new(9) {
            return Err("Version should be 9 after migration.");
        }
        Ok(())
    }
}
