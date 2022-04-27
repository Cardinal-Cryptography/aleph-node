#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::{StorageDoubleMap, ValueQuery},
        Twox64Concat,
    };
    use primitives::SessionIndex;

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Whether validator was in committee at session.
    #[pallet::storage]
    #[pallet::getter(fn eras_participated_sessions)]
    pub type SessionParticipated<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        SessionIndex,
        Twox64Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;
}
