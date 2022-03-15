//! This pallet is an on-chain light-client for tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, performed by a so-called relayer
//! It is a part of the Aleph0 <-> Terra bridge

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
mod utils;

use frame_support::traits::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::{
        types::{BridgedBlockHash, LightBlockStorage, LightClientOptionsStorage},
        utils::from_unix_timestamp,
    };
    use frame_support::{
        ensure, fail, log,
        pallet_prelude::{
            DispatchClass, DispatchResult, IsType, OptionQuery, StorageMap, StorageValue,
            ValueQuery,
        },
        traits::{Get, UnixTime},
        Identity,
    };
    use frame_system::{ensure_root, ensure_signed, pallet_prelude::OriginFor};
    use sp_std::vec::Vec;
    use tendermint::Time;
    use tendermint_light_client_verifier::{
        options::Options, types::LightBlock, ProdVerifier, Verifier,
    };

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// ubiquitous event type
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Maximal number of finalized headers to keep in the storage, last-in first-out
        #[pallet::constant]
        type HeadersToKeep: Get<u32>;

        /// time provider type, used to gauge whther blocks are within the trusting period
        type TimeProvider: UnixTime;
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
        /// Light client has not been initialized        
        NotInitialized,
        /// Light client has already been initialized
        AlreadyInitialized,
        /// Light client is currently halted
        Halted,
        /// The minimum voting power threshold is not reached, the block cannot be trusted yet
        NotEnoughTrust,
        /// Verification failed, the block is invalid.        
        InvalidBlock,
    }

    // TODOs for storage:
    // - header storage should be a ring buffer (i.e. we keep n last headers, ordered by the insertion time)
    // - we keep a pointer to the last finalized header (to avoid deserializing the whole buffer)
    // - insertion moves the pointer and updates the buffer
    // or?
    // https://substrate.recipes/ringbuffer.html

    /// Hash of the best finalized header from the bridged chain
    #[pallet::storage]
    pub type BestFinalized<T: Config> = StorageValue<_, BridgedBlockHash, ValueQuery>;

    /// A ring buffer of imported hashes ordered by their insertion time
    #[pallet::storage]
    pub type ImportedHashes<T: Config> = StorageMap<_, Identity, u32, BridgedBlockHash>;

    /// Current ring buffer position
    #[pallet::storage]
    pub(super) type ImportedHashesPointer<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Bridged chain Headers which have been imported by the client
    /// Client keeps HeadersToKeep number of these at any time
    #[pallet::storage]
    pub(super) type ImportedBlocks<T: Config> =
        StorageMap<_, Identity, BridgedBlockHash, LightBlockStorage, OptionQuery>;

    // END: TODOs

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
        pub fn initialize_client(
            origin: OriginFor<T>,
            options: LightClientOptionsStorage,
            initial_block: LightBlockStorage,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // ensure client is not already initialized
            let can_initialize = !<BestFinalized<T>>::exists();
            ensure!(can_initialize, <Error<T>>::AlreadyInitialized);

            // let options: LightClientOptionsStorage = serde_json::from_slice(&options_payload[..])
            //     .map_err(|e| {
            //     log::error!("Error when deserializing options: {}", e);
            //     Error::<T>::DeserializeError
            // })?;

            <LightClientOptions<T>>::put(options);

            // let light_block: LightBlockStorage = serde_json::from_slice(&initial_block_payload[..])
            //     .map_err(|e| {
            //         log::error!(target: "runtime::tendermint-lc","Error when deserializing initial light block: {}", e);
            //         Error::<T>::DeserializeError
            //     })?;

            let hash = initial_block.signed_header.commit.block_id.hash.clone();
            <ImportedHashesPointer<T>>::put(0);
            // update block storage
            insert_light_block::<T>(hash, initial_block.clone());

            // update status
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
            let _ = ensure_signed(origin)?;

            let options: Options = <LightClientOptions<T>>::get().try_into()?;

            let verifier = ProdVerifier::default();

            let untrusted_block_storage: LightBlockStorage = serde_json::from_slice(&untrusted_block_payload[..])
                .map_err(|e| {
                    log::error!(target: "runtime::tendermint-lc", "Error when deserializing light block: {}", e);
                    Error::<T>::DeserializeError
                })?;

            log::debug!(target: "runtime::tendermint-lc", "Verifying light block {:?}", &untrusted_block_storage);

            let most_recent_trusted_block: LightBlock = match <ImportedBlocks<T>>::get(
                <BestFinalized<T>>::get(),
            ) {
                Some(best_finalized) => best_finalized.try_into().expect(
                    "Unexpected failure when casting most recent trusted block as a LightBlock",
                ),
                None => {
                    log::error!(
                        target: "runtime::tendermint-lc",
                        "Cannot finalize light block {:?} because Light Client is not yet initialized",
                        &untrusted_block_storage,
                    );
                    fail!(<Error<T>>::NotInitialized);
                }
            };

            let now = T::TimeProvider::now();
            let time_now: Time = from_unix_timestamp(now.as_secs().try_into().unwrap());

            let untrusted_block: LightBlock = untrusted_block_storage
                .clone()
                .try_into()
                .expect("Unexpected failure when casting unstrusted block as a LightBlock");

            // verify against known state
            let verdict = verifier.verify(
                untrusted_block.as_untrusted_state(),
                most_recent_trusted_block.as_trusted_state(),
                &options,
                time_now,
            );

            match verdict {
                tendermint_light_client_verifier::Verdict::Success => {
                    // update storage
                    let hash = untrusted_block_storage
                        .signed_header
                        .commit
                        .block_id
                        .hash
                        .clone();
                    insert_light_block::<T>(hash, untrusted_block_storage);

                    Ok(())
                }
                tendermint_light_client_verifier::Verdict::NotEnoughTrust(_) => {
                    fail!(<Error<T>>::NotEnoughTrust)
                }
                tendermint_light_client_verifier::Verdict::Invalid(_) => {
                    fail!(<Error<T>>::InvalidBlock)
                }
            }
        }

        /// Halt or resume all light client operations
        ///
        /// Can only be called by root
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn set_halted(origin: OriginFor<T>, halted: bool) -> DispatchResult {
            ensure_root(origin)?;
            <IsHalted<T>>::put(halted);

            if halted {
                log::warn!(target: "runtime::tendermint-lc", "Halting light client operations");
                Self::deposit_event(Event::LightClientHalted);
            } else {
                log::warn!(target: "runtime::tendermint-lc", "Resuming light client operations");
                Self::deposit_event(Event::LightClientResumed);
            }

            Ok(())
        }
    }

    /// update light client storage
    /// should only be called by a trusted origin, *after* performing a verification
    fn insert_light_block<T: Config>(hash: Vec<u8>, light_block: LightBlockStorage) {
        let index = <ImportedHashesPointer<T>>::get();
        let pruning = <ImportedHashes<T>>::try_get(index);

        <BestFinalized<T>>::put(hash.clone());
        <ImportedBlocks<T>>::insert(hash.clone(), light_block);
        <ImportedHashes<T>>::insert(index, hash.clone());
        <ImportedHashesPointer<T>>::put((index + 1) % T::HeadersToKeep::get());

        // prune light block
        if let Ok(hash) = pruning {
            log::info!(target: "runtime::tendermint-lc", "Pruninig a stale light block with hash {:?}", hash);
            <ImportedBlocks<T>>::remove(hash);
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
