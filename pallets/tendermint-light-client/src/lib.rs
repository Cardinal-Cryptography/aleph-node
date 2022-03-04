//! This pallet is an on-chain light-client for tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, performed by a so-called relayer
//! It is a part of the Aleph0 <-> Terra bridge

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        log,
        pallet_prelude::{DispatchClass, DispatchResult, IsType, StorageValue, ValueQuery},
        traits::Get,
    };
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use tendermint_light_client_verifier::types::LightBlock;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // TODO events

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pallet is halted
        LightClientHalted,
        /// Pallet operations are resumed        
        LightClientResumed,
        /// light client is initialized
        ClientInitialized(u32),
    }

    // TODO : errors

    #[pallet::error]
    pub enum Error<T> {
        NotInitialized,
        /// light client has already been initialized
        AlreadyInitialized,
        /// light client is halted
        Halted,
    }

    // TODO : storage

    /// If true, stop the world
    #[pallet::storage]
    #[pallet::getter(fn is_halted)]
    pub type IsHalted<T: Config> = StorageValue<_, bool, ValueQuery>;

    // TODO : calls

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // TODO : adjust weight
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn initialize_client(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            // TODO

            Ok(())
        }

        // TODO : adjust weight
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn submit_finality_proof(origin: OriginFor<T>) -> DispatchResult {
            ensure_not_halted::<T>()?;

            // TODO : types for justification and header

            // TODO : verify against known state

            // TODO : udpate storage

            Ok(())
        }

        /// Halt or resume all operations.
        ///
        /// Can only be called by root
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn set_halted(origin: OriginFor<T>, halted: bool) -> DispatchResult {
            ensure_root(origin)?;
            <IsHalted<T>>::put(halted);

            if halted {
                log::info!(target: "runtime::tendermint-lc", "Halting light client operations");
                Self::deposit_event(Event::LightClientHalted);
            } else {
                log::warn!(target: "runtime::tendermint-lc", "Resuming light client operations.");
                Self::deposit_event(Event::LightClientResumed);
            }

            Ok(())
        }
    }

    /// Ensure that the bridge is not in a halted state
    fn ensure_not_halted<T: Config>() -> Result<(), Error<T>> {
        if <IsHalted<T>>::get() {
            Err(<Error<T>>::Halted)
        } else {
            Ok(())
        }
    }
}
