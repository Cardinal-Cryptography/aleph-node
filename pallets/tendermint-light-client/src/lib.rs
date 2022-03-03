//! This pallet is an on-chain light-client for tendermint (Cosmos) based chains
//! It verifies headers submitted to it via on-chain transactions, usually performed by a relayer
//! It is a part of Aleph0 <-> Terra bridge

#![cfg_attr(not(feature = "std"), no_std)]
use sp_std::prelude::*;

use frame_support::{
    log,
    sp_runtime::BoundToRuntimeAppPublic,
    traits::{OneSessionHandler, StorageVersion},
    Parameter,
};
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, sp_runtime::RuntimeAppPublic};
    use frame_system::pallet_prelude::BlockNumberFor;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);
}
