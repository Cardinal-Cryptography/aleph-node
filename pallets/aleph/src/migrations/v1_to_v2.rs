use crate::Config;
use frame_support::log;
use frame_support::{
    generate_storage_alias,
    traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
    weights::Weight,
};

generate_storage_alias!(Aleph, SessionForValidatorsChange => Value<()>);
generate_storage_alias!(Aleph, MillisecsPerBlock => Value<()>);
generate_storage_alias!(Aleph, SessionPeriod => Value<()>);

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let current_storage_version = <P as GetStorageVersion>::current_storage_version();

    if on_chain_storage_version == 1 && current_storage_version == 2 {
        let mut writes = 0;
        log::info!(target: "pallet_aleph", "Running migration from STORAGE_VERSION 1 to 2");

        if !SessionForValidatorsChange::exists() {
            log::info!(target: "pallet_aleph", "Storage item SessionForValidatorsChange does not exist!");
        } else {
            writes += 1;
        }
        SessionForValidatorsChange::kill();

        if !MillisecsPerBlock::exists() {
            log::info!(target: "pallet_aleph", "Storage item MillisecsPerBlock does not exist!");
        } else {
            writes += 1;
        }
        MillisecsPerBlock::kill();

        if !SessionPeriod::exists() {
            log::info!(target: "pallet_aleph", "Storage item SessionPeriod does not exist!");
        } else {
            writes += 1;
        }
        SessionPeriod::kill();

        // store new version
        StorageVersion::new(2).put::<P>();
        writes += 1;

        T::DbWeight::get().reads(4) + T::DbWeight::get().writes(writes)
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
