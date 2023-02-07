use frame_support::{pallet_prelude::StorageVersion, traits::OnRuntimeUpgrade};
use pallet_session::historical::{Config, Pallet};
use sp_runtime::traits::Get;

use crate::{log, Weight};

pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
    fn on_runtime_upgrade() -> Weight {
        StorageVersion::new(1).put::<Pallet<T>>();
        log::info!(target: "runtime::historical", "migrated to V1");
        T::DbWeight::get().reads_writes(0, 1)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == 0 || StorageVersion::get::<Pallet<T>>() == 1,
            "💸 Migration being executed on the wrong storage \
                version, expected 0 or 1"
        );

        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == 1,
            "💸 must upgrade to v1"
        );

        Ok(())
    }
}
