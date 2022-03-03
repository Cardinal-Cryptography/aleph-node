//! This pallet is an on-chain light-client for tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, performed by a so-called relayer
//! It is a part of the Aleph0 <-> Terra bridge

#![cfg_attr(not(feature = "std"), no_std)]
// use sp_std::prelude::*;

// use frame_support::{
//     log,
//     sp_runtime::BoundToRuntimeAppPublic,
//     traits::{OneSessionHandler, StorageVersion},
//     Parameter,
// };
// pub use pallet::*;

use frame_support::traits::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::{IsType, StorageValue, ValueQuery};

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
        /// light client is initialized
        ClientInitialized(u32),
    }

    // TODO : storage

    /// If true, stop the world
    #[pallet::storage]
    #[pallet::getter(fn is_halted)]
    pub type IsHalted<T: Config> = StorageValue<_, bool, ValueQuery>;

    // TODO : calls
}
