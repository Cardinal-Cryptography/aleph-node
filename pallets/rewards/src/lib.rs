#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        Twox64Concat,
        pallet_prelude::{StorageDoubleMap, ValueQuery}
    };
    use pallet_staking::EraIndex;

    #[pallet::config]
    pub trait Config: frame_system::Config {
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Number of block produced by validator at era.
    #[pallet::storage]
    #[pallet::getter(fn eras_block_to_produce)]
    pub type ErasBlockProduced<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        EraIndex,
        Twox64Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Number of sessions in which the validator was in committee at era.
    #[pallet::storage]
    #[pallet::getter(fn eras_participated_sessions)]
    pub type ErasParticipatedSessions<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        EraIndex,
        Twox64Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;
}