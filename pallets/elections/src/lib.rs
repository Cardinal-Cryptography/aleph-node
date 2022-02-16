#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use frame_support::traits::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_election_provider_support::{
        ElectionDataProvider, ElectionProvider, Support, Supports,
    };
    use frame_support::pallet_prelude::*;
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use primitives::{
        SessionIndex, DEFAULT_MILLISECS_PER_BLOCK, DEFAULT_SESSIONS_PER_ERA, DEFAULT_SESSION_PERIOD,
    };
    use sp_std::{collections::btree_map::BTreeMap, prelude::Vec};

    #[pallet::storage]
    #[pallet::getter(fn members)]
    pub type Members<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type DataProvider: ElectionDataProvider<Self::AccountId, Self::BlockNumber>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ChangeMembers(Vec<T::AccountId>),
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn change_members(origin: OriginFor<T>, members: Vec<T::AccountId>) -> DispatchResult {
            ensure_root(origin)?;
            Members::<T>::put(members.clone());
            Self::deposit_event(Event::ChangeMembers(members));

            Ok(())
        }
    }

    #[pallet::type_value]
    pub(super) fn DefaultForSessionsPerEra() -> u32 {
        DEFAULT_SESSIONS_PER_ERA
    }

    #[pallet::storage]
    #[pallet::getter(fn sessions_per_era)]
    pub(super) type SessionsPerEra<T: Config> =
        StorageValue<_, SessionIndex, ValueQuery, DefaultForSessionsPerEra>;

    #[pallet::type_value]
    pub(super) fn DefaultForSessionPeriod() -> u32 {
        DEFAULT_SESSION_PERIOD
    }

    #[pallet::storage]
    #[pallet::getter(fn session_period)]
    pub(super) type SessionPeriod<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultForSessionPeriod>;

    #[pallet::type_value]
    pub(super) fn DefaultForMillisecsPerBlock() -> u64 {
        DEFAULT_MILLISECS_PER_BLOCK
    }

    #[pallet::storage]
    #[pallet::getter(fn millisecs_per_block)]
    pub(super) type MillisecsPerBlock<T: Config> =
        StorageValue<_, u64, ValueQuery, DefaultForMillisecsPerBlock>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub members: Vec<T::AccountId>,
        pub millisecs_per_block: u64,
        pub session_period: u32,
        pub sessions_per_era: SessionIndex,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                members: Vec::new(),
                millisecs_per_block: DEFAULT_MILLISECS_PER_BLOCK,
                session_period: DEFAULT_SESSION_PERIOD,
                sessions_per_era: DEFAULT_SESSIONS_PER_ERA,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <Members<T>>::put(&self.members);
            <MillisecsPerBlock<T>>::put(&self.millisecs_per_block);
            <SessionPeriod<T>>::put(&self.session_period);
            <SessionsPerEra<T>>::put(&self.sessions_per_era);
        }
    }

    impl<T: Config> Pallet<T> {}

    #[derive(Debug)]
    pub enum Error {
        DataProvider(&'static str),
    }

    impl<T: Config> ElectionProvider<T::AccountId, BlockNumberFor<T>> for Pallet<T> {
        type Error = Error;
        type DataProvider = T::DataProvider;

        // We use authority list so the supports don't influance the result
        // but we still have to calculate them for the sake of eras payouts
        fn elect() -> Result<Supports<T::AccountId>, Self::Error> {
            let voters = Self::DataProvider::voters(None).map_err(Error::DataProvider)?;
            let members = Pallet::<T>::members();
            let mut supports: BTreeMap<T::AccountId, Support<T::AccountId>> = members
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        Support {
                            total: 0,
                            voters: Vec::new(),
                        },
                    )
                })
                .collect();

            for (voter, vote, targets) in voters {
                // The parameter Staking::MAX_NOMINATIONS is set to 1 which guarantees that len(targets) == 1
                let member = &targets[0];
                if let Some(support) = supports.get_mut(member) {
                    support.total += vote as u128;
                    support.voters.push((voter, vote as u128));
                }
            }

            Ok(supports.into_iter().collect())
        }
    }
}
