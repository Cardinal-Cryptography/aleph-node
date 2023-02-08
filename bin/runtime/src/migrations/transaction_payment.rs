use codec::{Decode, Encode};
use frame_support::{
    dispatch::Weight,
    log,
    pallet_prelude::{Get, TypeInfo},
    storage_alias,
    traits::OnRuntimeUpgrade,
    RuntimeDebug,
};
use pallet_transaction_payment::Config;

/// Storage releases of the pallet.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
enum Releases {
    /// Original version of the pallet.
    V1Ancient,
    /// One that bumps the usage to FixedU128 from FixedI128.
    V2,
}

#[storage_alias]
type StorageVersion = StorageValue<TransactionPayment, Releases>;

pub struct MigrateToV2<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MigrateToV2<T> {
    fn on_runtime_upgrade() -> Weight {
        match StorageVersion::get() {
            None => {
                log::info!(
                    target: "runtime::transaction-payment",
                    "💸 Migrating storage to Releases::V2 from unknown version"
                );
                StorageVersion::put(Releases::V2);
                T::DbWeight::get().reads_writes(1, 1)
            }
            Some(Releases::V1Ancient) => {
                log::info!(
                    target: "runtime::transaction-payment",
                    "💸 Migrating storage to Releases::V2 from Releases::V1Ancient"
                );
                StorageVersion::put(Releases::V2);
                T::DbWeight::get().reads_writes(1, 1)
            }
            _ => {
                log::warn!(
                    target: "runtime::transaction-payment",
                    "💸 Migration being executed on the wrong storage \
                    version, expected Releases::V1Ancient or None"
                );
                T::DbWeight::get().reads(1)
            }
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get() == Some(Releases::V1Ancient) || StorageVersion::get() == None,
            "💸 Migration being executed on the wrong storage \
                version, expected Releases::V1Ancient or None"
        );

        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get() == Some(Releases::V2),
            "💸 must upgrade to Releases::V2"
        );

        Ok(())
    }
}
