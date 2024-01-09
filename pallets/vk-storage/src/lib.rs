//! # VK Storage pallet
//!
//! This pallet provides a way to store verification keys that can be used in the SNARK verification process. Anybody
//! can register a verification key. A key is stored in a map under its Blake256 hash. Pallet doesn't provide any way
//! for removing keys from the map, so it's a good idea to impose some costs on storing a key (see `StorageCharge`) to
//! avoid bloating the storage.
//!
//! For technical reasons, the keys are stored together with the SNARK setup parameter `k`, which denotes the logarithm
//! of the maximum number of rows in a supported circuit.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod tests;
mod weights;

use frame_support::{
    pallet_prelude::{RuntimeDebug, StorageVersion, Weight},
    sp_runtime::traits::BlakeTwo256,
    traits::Get,
    BoundedVec,
};
pub use pallet::*;
use sp_core::H256;
pub use weights::{AlephWeight, WeightInfo};

/// Hashing algorithm used for computing key hashes.
pub type KeyHasher = BlakeTwo256;
/// Hash type used for storing keys.
pub type KeyHash = H256;

/// Data stored by the pallet.
#[derive(
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    codec::Encode,
    codec::Decode,
    codec::MaxEncodedLen,
    scale_info::TypeInfo,
)]
#[scale_info(skip_type_params(KeyLenBound))]
pub struct StorageData<KeyLenBound: Get<u32>> {
    /// Verification key.
    ///
    /// Note that the pallet doesn't validate the key in any way, so it can be just a random sequence of bytes.
    pub key: BoundedVec<u8, KeyLenBound>,
    /// The logarithm of the maximum number of rows in a supported circuit.
    pub k: u32,
}

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};
    use sp_std::prelude::Vec;

    use super::*;
    use crate::StorageCharge;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Item required for emitting events.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Weight information for the pallet's extrinsics.
        type WeightInfo: WeightInfo;

        /// Limits how many bytes verification key can have.
        #[pallet::constant]
        type MaximumKeyLength: Get<u32>;

        /// The policy on charging for storing a key (in addition to the standard operation costs).
        #[pallet::constant]
        type StorageCharge: Get<StorageCharge>;
    }

    #[pallet::error]
    #[derive(Clone, Eq, PartialEq)]
    pub enum Error<T> {
        /// Provided verification key is longer than `MaximumKeyLength` limit.
        VerificationKeyTooLong,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Verification key has been successfully stored.
        VerificationKeyStored(KeyHash),
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type VerificationKeys<T: Config> =
        StorageMap<_, Twox64Concat, KeyHash, StorageData<T::MaximumKeyLength>>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Stores a pair (`key`, `k`) under the Blake256 hash of `key` in `VerificationKeys` map.
        ///
        /// # Errors
        ///
        /// This call will return an error if `key.len()` is greater than `MaximumKeyLength` limit.
        ///
        /// # Notes
        ///
        /// 1. `key` can come from any proving system - there are no checks that verify it, in
        /// particular, `key` can contain just trash bytes.
        ///
        /// 2. If the key is already stored, this call will succeed and charge the full weight, even though the whole
        /// work could have been avoided.
        ///
        /// 3. For performance reason, `k` is not taken into account when computing the key hash. `key` should be
        /// dependent on `k` anyway.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::store_key(key.len() as u32) + T::StorageCharge::get().charge_for(key.len()))]
        pub fn store_key(origin: OriginFor<T>, key: Vec<u8>, k: u32) -> DispatchResult {
            ensure_signed(origin)?;

            ensure!(
                key.len() <= T::MaximumKeyLength::get() as usize,
                Error::<T>::VerificationKeyTooLong
            );

            // We do not include `k` in hashing, because it would require cloning `key`.
            let hash = KeyHasher::hash(&key);
            VerificationKeys::<T>::insert(
                hash,
                StorageData {
                    key: BoundedVec::try_from(key)
                        .expect("Key is already guaranteed to be within length limits."),
                    k,
                },
            );

            Self::deposit_event(Event::VerificationKeyStored(hash));
            Ok(())
        }
    }
}

/// A simple linear model for charging for storing a key.
///
/// This should be used to impose higher costs on storing anything in this pallet (since there is no way of clearing
/// the storage). The costs should be charged in addition to the standard operation costs (i.e., database costs).
#[derive(
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    codec::Encode,
    codec::Decode,
    codec::MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub struct StorageCharge {
    base: u64,
    per_byte: u64,
}

impl StorageCharge {
    /// Creates a new charge model of a fixed cost.
    pub const fn constant(base: u64) -> Self {
        Self { base, per_byte: 0 }
    }

    /// Creates a new charge model of a linear cost.
    pub const fn linear(base: u64, per_byte: u64) -> Self {
        Self { base, per_byte }
    }

    /// Computes the fee for storing `bytes` bytes.
    pub fn charge_for(&self, bytes: usize) -> Weight {
        Weight::from_parts(
            self.base
                .saturating_add(self.per_byte.saturating_mul(bytes as u64)),
            0,
        )
    }
}
