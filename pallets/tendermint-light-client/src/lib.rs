//! This pallet is an on-chain light-client for tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, performed by a so-called relayer
//! It is a part of the Aleph0 <-> Terra bridge

#![cfg_attr(not(feature = "std"), no_std)]

mod types;
mod utils;

use frame_support::traits::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::types::{BridgedBlockHash, LightBlockStorage};
    use frame_support::{
        log,
        pallet_prelude::{
            DispatchClass, DispatchResult, IsType, StorageMap, StorageValue, ValueQuery,
        },
        traits::Get,
        Identity,
    };
    use frame_system::{ensure_root, pallet_prelude::OriginFor};
    use sp_std::vec::Vec;
    use tendermint_light_client_verifier::{options::Options, types::LightBlock, ProdVerifier};
    use types::LightClientOptionsStorage;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        // #[pallet::constant]
        // type ValidatorSetTrustThreshold: Get<TrustThresholdFraction>;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pallet is halted
        LightClientHalted,
        /// Pallet operations are resumed        
        LightClientResumed,
        /// light client is initialized
        LightClientInitialized,
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Unable to deserialize extrinsic
        DeserializeError,
        /// light client has not been initialized        
        NotInitialized,
        /// light client has already been initialized
        AlreadyInitialized,
        /// light client is halted
        Halted,
    }

    // TODOs for storage:
    // - header storage should be a ring buffer (i.e. we keep n last headers, ordered by the insertion time)
    // - we keep a pointer to the last finalized header (to avoid deserializing the whole buffer)
    // - insertion moves the pointer and updates the buffer

    /// Hash of the best finalized header from the bridged chain
    #[pallet::storage]
    pub type BestFinalized<T: Config> = StorageValue<_, BridgedBlockHash, ValueQuery>;

    /// A buffer of imported hashes ordered by their insertion time
    #[pallet::storage]
    pub type ImportedHashes<T: Config> = StorageMap<_, Identity, u32, BridgedBlockHash>;

    /// Current ring buffer position
    #[pallet::storage]
    pub(super) type ImportedHashesPointer<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Bridged chain Headers which have been imported by the client
    #[pallet::storage]
    pub(super) type ImportedHeaders<T: Config> =
        StorageMap<_, Identity, BridgedBlockHash, LightBlockStorage>;

    // END: TODO

    /// If true, stop the world
    #[pallet::storage]
    #[pallet::getter(fn is_halted)]
    pub type IsHalted<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn get_options)]
    pub type LightClientOptions<T: Config> = StorageValue<_, LightClientOptionsStorage, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // TODO : adjust weight
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn initialize_client(origin: OriginFor<T>, options_payload: Vec<u8>) -> DispatchResult {
            ensure_root(origin)?;

            let options: LightClientOptionsStorage = serde_json::from_slice(&options_payload[..])
                .map_err(|e| {
                log::error!("Error when deserializing options: {}", e);
                Error::<T>::DeserializeError
            })?;

            <LightClientOptions<T>>::put(options);
            <IsHalted<T>>::put(false);
            log::info!(target: "runtime::tendermint-lc", "Light client initialized");
            Self::deposit_event(Event::LightClientInitialized);

            Ok(())
        }

        // TODO : adjust weight
        /// Verify a block header against a known state.        
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn submit_finality_proof(
            origin: OriginFor<T>,
            untrusted_block_payload: Vec<u8>,
        ) -> DispatchResult {
            ensure_not_halted::<T>()?;

            let options: Options = <LightClientOptions<T>>::get().try_into()?;

            let verifier = ProdVerifier::default();

            // TODO : storage type for Light Block
            let untrusted_state: LightBlock = serde_json::from_slice(&untrusted_block_payload[..])
                .map_err(|e| {
                    log::error!("Error when deserializing light block: {}", e);
                    Error::<T>::DeserializeError
                })?;

            // TODO : verify against known state

            // TODO : update storage

            Ok(())
        }

        /// Halt or resume all light client operations
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
                log::warn!(target: "runtime::tendermint-lc", "Resuming light client operations");
                Self::deposit_event(Event::LightClientResumed);
            }

            Ok(())
        }
    }

    /// Ensure that the light client is not in a halted state
    fn ensure_not_halted<T: Config>() -> Result<(), Error<T>> {
        if <IsHalted<T>>::get() {
            Err(<Error<T>>::Halted)
        } else {
            Ok(())
        }
    }
}
