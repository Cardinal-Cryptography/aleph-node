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

use sp_std::prelude::*;

use frame_support::{
    sp_runtime::BoundToRuntimeAppPublic,
    traits::{OneSessionHandler, StorageVersion},
    Parameter,
};
pub use pallet::*;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, sp_runtime::RuntimeAppPublic};
    use frame_system::pallet_prelude::BlockNumberFor;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type AuthorityId: Member
            + Parameter
            + RuntimeAppPublic
            + Default
            + MaybeSerializeDeserialize;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            let mut weight = 0;
            weight += migrations::v0_to_v1::migrate::<T, Self>();
            weight += migrations::v1_to_v2::migrate::<T, Self>();
            weight
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

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

        fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            let (_, authorities): (Vec<_>, Vec<_>) = validators.unzip();
            Self::update_authorities(authorities.as_slice());
        }

        fn on_disabled(_validator_index: u32) {}
    }
}
