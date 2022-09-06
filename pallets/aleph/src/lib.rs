//! This pallet is a runtime companion of Aleph finality gadget.
//!
//! Currently, it only provides support for changing sessions but in the future
//! it will allow reporting equivocation in AlephBFT.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod migrations;

use frame_support::{
    log,
    sp_runtime::BoundToRuntimeAppPublic,
    traits::{OneSessionHandler, StorageVersion},
};
pub use pallet::*;
use primitives::{SessionIndex, Version, VersionChange};
use sp_std::prelude::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{pallet_prelude::*, sp_runtime::RuntimeAppPublic};
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use pallets_support::StorageMigration;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type AuthorityId: Member + Parameter + RuntimeAppPublic + MaybeSerializeDeserialize;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ChangeEmergencyFinalizer(T::AuthorityId),
        ChangeAlephBFTVersion(VersionChange),
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            let on_chain = <Pallet<T> as GetStorageVersion>::on_chain_storage_version();
            T::DbWeight::get().reads(1)
                + match on_chain {
                    _ if on_chain == STORAGE_VERSION => 0,
                    _ if on_chain == StorageVersion::new(1) => {
                        migrations::v1_to_v2::Migration::<T, Self>::migrate()
                    }
                    _ if on_chain == StorageVersion::new(0) => {
                        migrations::v0_to_v1::Migration::<T, Self>::migrate()
                            + migrations::v1_to_v2::Migration::<T, Self>::migrate()
                    }
                    _ => {
                        log::warn!(
                            target: "pallet_aleph",
                            "On chain storage version of pallet aleph is {:?} but it should not be bigger than 2",
                            on_chain
                        );
                        0
                    }
                }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn emergency_finalizer)]
    pub(super) type EmergencyFinalizer<T: Config> = StorageValue<_, T::AuthorityId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn queued_emergency_finalizer)]
    pub(super) type QueuedEmergencyFinalizer<T: Config> =
        StorageValue<_, T::AuthorityId, OptionQuery>;

    #[pallet::storage]
    type NextEmergencyFinalizer<T: Config> = StorageValue<_, T::AuthorityId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn aleph_bft_version)]
    pub(super) type AlephBFTVersion<T: Config> = StorageValue<_, VersionChange, ValueQuery>;

    impl<T: Config> Pallet<T> {
        pub(crate) fn initialize_authorities(authorities: &[T::AuthorityId]) {
            if !authorities.is_empty() {
                assert!(
                    <Authorities<T>>::get().is_empty(),
                    "Authorities are already initialized!"
                );
                <Authorities<T>>::put(authorities);
            }
        }

        pub(crate) fn update_authorities(authorities: &[T::AuthorityId]) {
            <Authorities<T>>::put(authorities);
        }

        pub(crate) fn update_emergency_finalizer() {
            if let Some(emergency_finalizer) = <QueuedEmergencyFinalizer<T>>::get() {
                <EmergencyFinalizer<T>>::put(emergency_finalizer)
            }

            if let Some(emergency_finalizer) = <NextEmergencyFinalizer<T>>::get() {
                <QueuedEmergencyFinalizer<T>>::put(emergency_finalizer)
            }
        }

        pub(crate) fn set_next_emergency_finalizer(emergency_finalizer: T::AuthorityId) {
            <NextEmergencyFinalizer<T>>::put(emergency_finalizer);
        }

        pub(crate) fn set_next_aleph_bft_version(version_change: VersionChange) {
            let previous_version_change = <AlephBFTVersion<T>>::get();
            let previous_session = previous_version_change.session_change;

            let session = version_change.session_change;

            assert!(
                session > previous_session,
                "Cannot set AlephBFT version for sessions prior to the current version! Session for current version: {}, attempted to set: {}.", previous_session, session);
            <AlephBFTVersion<T>>::put(version_change);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sets the emergency finalization key. If called in session `N` the key can be used to
        /// finalize blocks from session `N+2` onwards, until it gets overridden.
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn set_emergency_finalizer(
            origin: OriginFor<T>,
            emergency_finalizer: T::AuthorityId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::set_next_emergency_finalizer(emergency_finalizer.clone());
            Self::deposit_event(Event::ChangeEmergencyFinalizer(emergency_finalizer));
            Ok(())
        }

        /// Sets AlephBFT version for a future session. If such a feature version is already set,
        /// it is replaced with the specified one.
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn set_aleph_bft_version(
            origin: OriginFor<T>,
            version: Version,
            session_change: SessionIndex,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let version_change = VersionChange {
                version,
                session_change,
            };
            Self::set_next_aleph_bft_version(version_change.clone());
            Self::deposit_event(Event::ChangeAlephBFTVersion(version_change));
            Ok(())
        }
    }

    impl<T: Config> BoundToRuntimeAppPublic for Pallet<T> {
        type Public = T::AuthorityId;
    }

    impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
        type Key = T::AuthorityId;

        fn on_genesis_session<'a, I: 'a>(validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            let (_, authorities): (Vec<_>, Vec<_>) = validators.unzip();
            Self::initialize_authorities(authorities.as_slice());
        }

        fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            Self::update_emergency_finalizer();
            if changed {
                let (_, authorities): (Vec<_>, Vec<_>) = validators.unzip();
                Self::update_authorities(authorities.as_slice());
            }
        }

        fn on_disabled(_validator_index: u32) {}
    }
}
