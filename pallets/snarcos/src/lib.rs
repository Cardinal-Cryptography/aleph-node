#![cfg_attr(not(feature = "std"), no_std)]

mod relation;

use frame_support::pallet_prelude::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::{ensure_root, pallet_prelude::OriginFor};

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ElSnarco,
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    impl<T: Config> Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(41)]
        pub fn summon_el_snarco(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            Self::deposit_event(Event::ElSnarco);
            Ok(())
        }
    }
}
