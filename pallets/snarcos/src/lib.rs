#![cfg_attr(not(feature = "std"), no_std)]

mod relation;

use frame_support::pallet_prelude::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

/// We store verification keys under short identifiers.
pub type VerificationKeyIdentifier = [u8; 4];

#[frame_support::pallet]
pub mod pallet {
    use ark_ec::PairingEngine;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;
    use sp_std::prelude::Vec;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Field: PairingEngine;

        #[pallet::constant]
        type MaximumVerificationKeyLength: Get<u32>;
    }

    #[pallet::error]
    pub enum Error<T> {
        IdentifierAlreadyInUse,
        VerificationKeyTooLong,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        VerificationKeyStored,
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type VerificationKeys<T: Config> = StorageMap<
        _,
        Twox64Concat,
        VerificationKeyIdentifier,
        BoundedVec<u8, T::MaximumVerificationKeyLength>,
    >;

    impl<T: Config> Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(41)]
        pub fn store_key(
            _origin: OriginFor<T>,
            identifier: VerificationKeyIdentifier,
            key: Vec<u8>,
        ) -> DispatchResult {
            ensure!(
                !VerificationKeys::<T>::contains_key(identifier.clone()),
                Error::<T>::IdentifierAlreadyInUse
            );
            ensure!(
                key.len() <= T::MaximumVerificationKeyLength::get() as usize,
                Error::<T>::VerificationKeyTooLong
            );

            VerificationKeys::<T>::insert(
                identifier,
                BoundedVec::try_from(key).unwrap(), // must succeed since we've just check length
            );

            Self::deposit_event(Event::VerificationKeyStored);
            Ok(())
        }
    }
}
