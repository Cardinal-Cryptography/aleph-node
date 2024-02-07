//! # Feature control pallet
//!
//! This pallet provides a way of turning on/off features in the runtime that cannot be controlled with runtime
//! configuration. It maintains a simple map of feature identifiers together with their status (enabled/disabled). It is
//! supposed to be modified only by the runtime's `sudo` origin, but read by any runtime code.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![deny(missing_docs)]

use frame_support::pallet_prelude::{StorageVersion, Weight};
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Item required for emitting events.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::error]
    #[derive(Clone, Eq, PartialEq)]
    pub enum Error<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}
