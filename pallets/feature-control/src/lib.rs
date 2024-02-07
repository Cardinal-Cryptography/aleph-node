//! # Feature control pallet
//!
//! This pallet provides a way of turning on/off features in the runtime that cannot be controlled with runtime
//! configuration. It maintains a simple map of feature identifiers together with their status (enabled/disabled). It is
//! supposed to be modified only by the specified origin, but read by any runtime code.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![deny(missing_docs)]

use frame_support::pallet_prelude::StorageVersion;
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

/// All available optional features for the Aleph Zero runtime.
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum Feature {
    /// The on-chain verifier feature involves:
    /// - VkStorage pallet (for storing verification keys)
    /// - smart contract chain extension exposing `verify` function
    /// - SnarkVerifier runtime interface
    #[codec(index = 0)]
    OnChainVerifier,
}

/// The status of a feature: either enabled or disabled.
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum FeatureStatus {
    /// The feature is enabled.
    #[codec(index = 0)]
    Enabled,
    /// The feature is disabled.
    #[codec(index = 1)]
    Disabled,
}

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;

    use super::{FeatureStatus::*, *};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Item required for emitting events.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The origin that can modify the feature map.
        type Controller: EnsureOrigin<Self::RuntimeOrigin>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A feature has been enabled or disabled.
        FeatureStatusChanged(Feature, FeatureStatus),
    }

    #[pallet::storage]
    pub type ActiveFeatures<T: Config> = StorageMap<_, Twox64Concat, Feature, ()>;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Enable a feature.
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn enable(origin: OriginFor<T>, feature: Feature) -> DispatchResult {
            T::Controller::ensure_origin(origin)?;
            ActiveFeatures::<T>::insert(feature, ());
            Self::deposit_event(Event::FeatureStatusChanged(feature, Enabled));
            Ok(())
        }

        /// Disable a feature.
        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn disable(origin: OriginFor<T>, feature: Feature) -> DispatchResult {
            T::Controller::ensure_origin(origin)?;
            ActiveFeatures::<T>::remove(feature);
            Self::deposit_event(Event::FeatureStatusChanged(feature, Disabled));
            Ok(())
        }
    }
}
