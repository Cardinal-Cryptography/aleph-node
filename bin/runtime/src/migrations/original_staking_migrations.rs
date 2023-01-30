use frame_support::{
    pallet_prelude::{Get, GetStorageVersion, StorageVersion},
    traits::{CrateVersion, OnRuntimeUpgrade, PalletInfoAccess},
};
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

use crate::Runtime;

pub struct StakingBagsListMigrationV8;

impl OnRuntimeUpgrade for StakingBagsListMigrationV8 {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        pallet_staking::migrations::v8::migrate::<Runtime>()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        pallet_staking::migrations::v8::pre_migrate::<Runtime>().map(|_| Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        pallet_staking::migrations::v8::post_migrate::<Runtime>()
    }
}

pub struct StakingMigrationV11OldPallet;
impl Get<&'static str> for StakingMigrationV11OldPallet {
    fn get() -> &'static str {
        "VoterList"
    }
}

impl PalletInfoAccess for StakingMigrationV11OldPallet {
    fn index() -> usize {
        // does not matter for  pallet_staking::migrations::v11::MigrateToV11
        todo!()
    }

    fn name() -> &'static str {
        "VoterList"
    }

    fn module_name() -> &'static str {
        // does not matter for  pallet_staking::migrations::v11::MigrateToV11
        todo!()
    }

    fn crate_version() -> CrateVersion {
        // does not matter for  pallet_staking::migrations::v11::MigrateToV11
        todo!()
    }
}

impl GetStorageVersion for StakingMigrationV11OldPallet {
    fn current_storage_version() -> StorageVersion {
        // does not matter for  pallet_staking::migrations::v11::MigrateToV11
        todo!()
    }

    fn on_chain_storage_version() -> StorageVersion {
        // does not matter for  pallet_staking::migrations::v11::MigrateToV11
        todo!()
    }
}
