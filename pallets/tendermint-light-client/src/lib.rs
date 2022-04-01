#![cfg_attr(not(feature = "std"), no_std)]

//! This pallet is an on-chain light-client for tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, performed by a so-called relayer
//! It is a part of the Aleph0 <-> Terra bridge
pub use pallet::*;

// #[cfg(test)]
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod types;
mod utils;

use frame_support::traits::StorageVersion;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::types::{BridgedBlockHash, LightBlockStorage, LightClientOptionsStorage};
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
        options::Options, types::LightBlock, ProdVerifier, Verifier,
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
        /// Pallet operations are resumed        
        LightClientResumed,
        /// Light client is initialized
        LightClientInitialized,
        /// New block has been verified and imported into storage \[relayer_address, imported_block_hash\]
        ImportedLightBlock(T::AccountId, BridgedBlockHash),
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

    // NOTE for storage:
    // - header storage should be a ring buffer (i.e. we keep n last headers, ordered by the insertion time)
    // - we keep a pointer to the last finalized header (to avoid deserializing the whole buffer)
    // - insertion moves the pointer and updates the buffer
    // or?
    // https://substrate.recipes/ringbuffer.html

    /// Hash of the last imported header from the bridged chain
    #[pallet::storage]
    #[pallet::getter(fn get_last_imported_hash)]
    pub type LastImportedHash<T: Config> = StorageValue<_, BridgedBlockHash, ValueQuery>;

    /// All imported hashes "ordered" by their insertion time
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

    // TODO : expose in runtime API and nodes RPC
    impl<T: Config> Pallet<T> {
        pub fn get_last_imported_block() -> Option<LightBlockStorage> {
            let ptr = ImportedHashesPointer::<T>::get()
                .checked_sub(1)
                .expect("unexpected failure when subtracting");

            match ImportedHashes::<T>::get(ptr) {
                Some(key) => ImportedBlocks::<T>::get(key),
                None => None,
            }
        }
    }

    /// If true, stop the world
    #[pallet::storage]
    #[pallet::getter(fn is_halted)]
    pub type IsHalted<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn get_options)]
    pub type LightClientOptions<T: Config> = StorageValue<_, LightClientOptionsStorage, ValueQuery>;

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
            let can_initialize = !<LastImportedHash<T>>::exists();
            ensure!(can_initialize, <Error<T>>::AlreadyInitialized);

            <LightClientOptions<T>>::put(options);

            let hash = initial_block.signed_header.commit.block_id.hash;
            <ImportedHashesPointer<T>>::put(0);
            // update block storage
            insert_light_block::<T>(hash, initial_block);

            // update status
            <IsHalted<T>>::put(false);
            log::info!(target: "runtime::tendermint-lc", "Light client initialized");
            Self::deposit_event(Event::LightClientInitialized);

            Ok(())
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

            log::debug!(target: "runtime::tendermint-lc", "Verifying light block {:#?}", &hash);

            let options: Options = <LightClientOptions<T>>::get().try_into()?;
            let verifier = ProdVerifier::default();
            let most_recent_trusted_block = match <ImportedBlocks<T>>::get(
                <LastImportedHash<T>>::get(),
            ) {
                Some(best_finalized) => best_finalized,
                None => {
                    log::error!(
                        target: "runtime::tendermint-lc",
                        "Cannot finalize light block {:?} because Light Client is not yet initialized",
                        &untrusted_block,
                    );
                    fail!(<Error<T>>::NotInitialized);
                }
            };

            let now = T::TimeProvider::now();
            let now = Time::from_unix_timestamp(now.as_secs().try_into().unwrap(), 0).unwrap();

            let verdict = verify_light_block(
                verifier,
                untrusted_block.clone(),
                most_recent_trusted_block,
                &options,
                now,
            );

            match verdict {
                tendermint_light_client_verifier::Verdict::Success => {
                    // update storage
                    insert_light_block::<T>(hash, untrusted_block);
                    log::info!(target: "runtime::tendermint-lc", "Successfully verified light block {:#?}", &hash);
                    Self::deposit_event(Event::ImportedLightBlock(who, hash));
                    Ok(())
                }
                tendermint_light_client_verifier::Verdict::NotEnoughTrust(voting_power_tally) => {
                    log::warn!(target: "runtime::tendermint-lc", "Not enough voting power to accept the light block {:#?}, vote tally  {}", &hash, &voting_power_tally);
                    fail!(<Error<T>>::NotEnoughTrust)
                }
                tendermint_light_client_verifier::Verdict::Invalid(why) => {
                    log::warn!(target: "runtime::tendermint-lc", "Rejecting invalid light block {:#?} becasue {}", &hash, &why);

                    fail!(<Error<T>>::InvalidBlock)
                }
            }
        }

        // TODO: This method will need to be called by the pallet itself if it detects a fork.
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

    fn verify_light_block(
        verifier: ProdVerifier,
        untrusted_block: LightBlockStorage,
        trusted_block: LightBlockStorage,
        options: &Options,
        now: Time,
    ) -> tendermint_light_client_verifier::Verdict {
        let untrusted_block: LightBlock = untrusted_block
            .try_into()
            .expect("Unexpected failure when casting untrusted block as tendermint::LightBlock");

        let trusted_block: LightBlock = trusted_block
            .try_into()
            .expect("Unexpected failure when casting trusted block as tendermint::LightBlock");

        // verify against known state
        verifier.verify(
            untrusted_block.as_untrusted_state(),
            trusted_block.as_trusted_state(),
            options,
            now,
        )
    }

    /// update light client storage
    /// should only be called by a trusted origin, *after* performing a verification
    fn insert_light_block<T: Config>(hash: BridgedBlockHash, light_block: LightBlockStorage) {
        let index = <ImportedHashesPointer<T>>::get();
        let pruning = <ImportedHashes<T>>::try_get(index);

        <LastImportedHash<T>>::put(hash);
        <ImportedBlocks<T>>::insert(hash, light_block);
        <ImportedHashes<T>>::insert(index, hash);
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
