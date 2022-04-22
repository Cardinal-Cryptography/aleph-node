#![cfg_attr(not(feature = "std"), no_std)]

//! This pallet is an on-chain light-client for Tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, performed by a so-called relayer
//! It is a part of the Aleph Zero <-> Tendermint bridge
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod generator;

pub mod types;
mod utils;

use frame_support::traits::StorageVersion;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::types::{
        ConversionError, LightBlockStorage, LightClientOptionsStorage, TendermintBlockHash,
        TendermintHashStorage,
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
    use tendermint::Time;
    use tendermint_light_client_verifier::{
        options::Options, types::LightBlock, ProdVerifier, Verdict, Verifier,
    };

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// ubiquitous event type
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Maximal number of finalized headers to keep in the storage, last-in first-out
        #[pallet::constant]
        type HeadersToKeep: Get<u32>;

        /// Maximal number of block validators in Tendermint
        #[pallet::constant]
        type MaxVotesCount: Get<u32>;

        /// time provider type, used to gauge whether blocks are within the trusting period
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
        /// client already in the same state as requested, indicates a no-op
        NoOp,
        /// Pallet operations are resumed        
        LightClientResumed,
        /// Light client is initialized
        LightClientInitialized,
        /// New block has been verified and imported into storage \[relayer_address, imported_block_hash, imported_block_height\]
        ImportedLightBlock(T::AccountId, TendermintBlockHash, u64),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Light client has not been initialized        
        NotInitialized,
        /// Light client has already been initialized
        AlreadyInitialized,
        /// Light client is currently halted
        Halted,
        /// The minimum voting power threshold is not reached, the block cannot be trusted yet
        NotEnoughTrust,
        /// Verification failed, the block is invalid        
        InvalidBlock,
        /// Initial block is invalid
        InvalidInitialBlock,
        /// Error during type conversion, usually indicative of a bug
        ConversionError,
        /// General error during client operations
        Other,
    }

    // NOTE for storage:
    // - header storage should be a ring buffer (i.e. we keep n last headers, ordered by the insertion time)
    // - we keep a pointer to the last finalized header (to avoid deserializing the whole buffer)
    // - insertion moves the pointer and updates the buffer
    // or?
    // https://substrate.recipes/ringbuffer.html

    /// Hash of the last imported header from the bridged chain
    #[pallet::storage]
    #[pallet::getter(fn get_last_imported_block_hash)]
    pub type LastImportedBlockHash<T: Config> = StorageValue<_, TendermintBlockHash, ValueQuery>;

    /// Imported hashes "ordered" by their insertion time
    /// Client keeps HeadersToKeep number of these at any time    
    #[pallet::storage]
    #[pallet::getter(fn get_imported_hash)]
    pub type ImportedHashes<T: Config> = StorageMap<_, Identity, u32, TendermintBlockHash>;

    /// Current ring buffer position
    #[pallet::storage]
    #[pallet::getter(fn get_imported_hashes_pointer)]
    pub(super) type ImportedHashesPointer<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Bridged chain Headers which have been imported by the client
    /// Client keeps HeadersToKeep number of these at any time
    #[pallet::storage]
    #[pallet::getter(fn get_imported_block)]
    pub(super) type ImportedBlocks<T: Config> =
        StorageMap<_, Identity, TendermintBlockHash, LightBlockStorage, OptionQuery>;

    impl<T: Config> Pallet<T> {
        pub fn get_last_imported_block() -> Option<LightBlockStorage> {
            match ImportedHashesPointer::<T>::get().checked_sub(1) {
                Some(ptr) => match ImportedHashes::<T>::get(ptr) {
                    Some(key) => ImportedBlocks::<T>::get(key),
                    None => None,
                },
                None => None,
            }
        }
    }

    /// If true, stop the world
    #[pallet::storage]
    #[pallet::getter(fn is_halted)]
    pub type IsHalted<T: Config> = StorageValue<_, bool, ValueQuery, DefaultIsHalted<T>>;

    #[pallet::type_value]
    pub fn DefaultIsHalted<T: Config>() -> bool {
        true
    }

    #[pallet::storage]
    #[pallet::getter(fn get_options)]
    pub type LightClientOptions<T: Config> =
        StorageValue<_, LightClientOptionsStorage, OptionQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // TODO : benchmark & adjust weights
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn initialize_client(
            origin: OriginFor<T>,
            options: LightClientOptionsStorage,
            initial_block: LightBlockStorage,
        ) -> DispatchResult {
            ensure_root(origin)?;

            log::debug!(target: "runtime::tendermint-lc", "Initializing Light Client light:\n options: {:?}\n initial block {:#?}", &options, &initial_block);

            // ensure client is not already initialized
            let can_initialize = !<LastImportedBlockHash<T>>::exists();
            ensure!(can_initialize, <Error<T>>::AlreadyInitialized);

            match initial_block.signed_header.commit.block_id.hash {
                TendermintHashStorage::Some(hash) => {
                    <LightClientOptions<T>>::put(options);
                    <ImportedHashesPointer<T>>::put(0);
                    // update block storage
                    insert_light_block::<T>(hash, initial_block);

                    // update status
                    <IsHalted<T>>::put(false);
                    log::info!(target: "runtime::tendermint-lc", "Light client initialized");
                    Self::deposit_event(Event::LightClientInitialized);
                    Ok(())
                }
                TendermintHashStorage::None => {
                    log::warn!(target: "runtime::tendermint-lc", "Rejecting invalid initial light block; empty hash");
                    fail!(<Error<T>>::InvalidInitialBlock)
                }
            }
        }

        // TODO : benchmark & adjust weights
        /// Verify a block header against a known state.        
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn update_client(
            origin: OriginFor<T>,
            untrusted_block: LightBlockStorage,
        ) -> DispatchResult {
            ensure_not_halted::<T>()?;
            let who = ensure_signed(origin)?;
            let hash = untrusted_block.signed_header.commit.block_id.hash;
            let height = untrusted_block.signed_header.header.height;

            log::debug!(target: "runtime::tendermint-lc", "Verifying light block {:#?} at height {}", &hash, &height);

            let options = match Self::get_options() {
                Some(options) => match options.try_into() {
                    Ok(options) => options,
                    Err(why) => {
                        log::error!(
                            target: "runtime::tendermint-lc",
                            "Cannot convert to Options {:?}",
                            &why,
                        );
                        fail!(<Error<T>>::ConversionError);
                    }
                },
                None => fail!(<Error<T>>::NotInitialized),
            };

            let verifier = ProdVerifier::default();
            let most_recent_trusted_block = match Self::get_imported_block(
                Self::get_last_imported_block_hash(),
            ) {
                Some(best_finalized) => match best_finalized.try_into() {
                    Ok(block) => block,
                    Err(why) => {
                        log::error!(target: "runtime::tendermint-lc", "Conversion failed {:?}", why);
                        fail!(<Error<T>>::ConversionError);
                    }
                },
                None => {
                    log::error!(
                        target: "runtime::tendermint-lc",
                        "Cannot finalize light block {:?} because Light Client is not yet initialized",
                        &untrusted_block,
                    );
                    fail!(<Error<T>>::NotInitialized);
                }
            };

            let seconds = T::TimeProvider::now().as_secs() as i64;
            let now = match Time::from_unix_timestamp(seconds, 0) {
                Ok(now) => now,
                Err(why) => {
                    log::error!(
                        target: "runtime::tendermint-lc",
                        "Cannot read current time {:?}",
                        &why,
                    );
                    fail!(<Error<T>>::Other)
                }
            };

            let to_verify = match untrusted_block.clone().try_into() {
                Ok(block) => block,
                Err(why) => {
                    log::error!(target: "runtime::tendermint-lc", "Conversion failed {:?}", why);
                    fail!(<Error<T>>::ConversionError);
                }
            };

            match verify_light_block::<T>(
                verifier,
                &to_verify,
                &most_recent_trusted_block,
                &options,
                now,
            ) {
                Verdict::Success => {
                    match hash {
                        TendermintHashStorage::Some(hash) => {
                            // update storage
                            insert_light_block::<T>(hash, untrusted_block);
                            log::info!(target: "runtime::tendermint-lc", "Successfully verified light block {:#?}", &hash);
                            Self::deposit_event(Event::ImportedLightBlock(who, hash, height));
                            Ok(())
                        }
                        TendermintHashStorage::None => fail!(<Error<T>>::InvalidBlock),
                    }
                }
                Verdict::NotEnoughTrust(voting_power_tally) => {
                    log::warn!(target: "runtime::tendermint-lc", "Not enough voting power to accept the light block {:#?}, vote tally  {}", &hash, &voting_power_tally);
                    fail!(<Error<T>>::NotEnoughTrust)
                }
                Verdict::Invalid(why) => {
                    log::warn!(target: "runtime::tendermint-lc", "Rejecting invalid light block {:#?} becasue {}", &hash, &why);
                    fail!(<Error<T>>::InvalidBlock)
                }
            }
        }

        // TODO: This method will need to be called by the pallet itself if it detects a fork.
        // TODO : weight depends on whether this is a no-op or not
        /// Halt or resume all light client operations
        ///
        /// Can only be called by root
        #[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
        pub fn set_halted(origin: OriginFor<T>, halted: bool) -> DispatchResult {
            ensure_root(origin)?;

            let current_status = Self::is_halted();

            match [halted, current_status].as_slice() {
                [true, true] | [false, false] => Self::deposit_event(Event::NoOp),
                [true, false] | [false, true] => {
                    <IsHalted<T>>::put(halted);
                    if halted {
                        log::warn!(target: "runtime::tendermint-lc", "Halting light client operations");
                        Self::deposit_event(Event::LightClientHalted);
                    } else {
                        log::warn!(target: "runtime::tendermint-lc", "Resuming light client operations");
                        Self::deposit_event(Event::LightClientResumed);
                    }
                }
                _ => fail!(<Error<T>>::Other),
            }

            Ok(())
        }
    }

    fn verify_light_block<T: Config>(
        verifier: ProdVerifier,
        untrusted_block: &LightBlock,
        trusted_block: &LightBlock,
        options: &Options,
        now: Time,
    ) -> tendermint_light_client_verifier::Verdict {
        // verify against trusted state
        verifier.verify(
            untrusted_block.as_untrusted_state(),
            trusted_block.as_trusted_state(),
            options,
            now,
        )
    }

    /// update light client storage
    fn insert_light_block<T: Config>(hash: TendermintBlockHash, light_block: LightBlockStorage) {
        let index = Pallet::<T>::get_imported_hashes_pointer();

        <LastImportedBlockHash<T>>::put(hash);
        <ImportedBlocks<T>>::insert(hash, light_block);
        <ImportedHashesPointer<T>>::put((index + 1) % T::HeadersToKeep::get());

        <ImportedHashes<T>>::mutate(index, |current| {
            // prune light block
            if let Some(hash) = current {
                log::info!(target: "runtime::tendermint-lc", "Pruninig a stale light block with hash {:?}", hash);
                <ImportedBlocks<T>>::remove(hash);
            }
            *current = Some(hash);
        });
    }

    /// Ensure that the light client is not in a halted state
    fn ensure_not_halted<T: Config>() -> Result<(), Error<T>> {
        if Pallet::<T>::is_halted() {
            Err(<Error<T>>::Halted)
        } else {
            Ok(())
        }
    }
}
