//! This pallet manages changes in the committee responsible for producing blocks and establishing consensus.
//! Currently, it's PoA where the validators are set by the root account. In the future, a new
//! version for DPoS elections will replace the current one.
//!
//! # Terminology
//! For definition of session, era, staking see pallet_session and pallet_staking.
//! - committee ([`EraValidators`]): Set of nodes that produce and finalize blocks in the session.
//! - validator: Node that can become a member of committee (or already is) via rotation.
//! - `EraValidators::reserved`: immutable validators, ie they cannot be removed from that list.
//! - `EraValidators::non_reserved`: validators that can be kicked out from that list
//!
//! # Kick out logic
//! In case of insufficient validator's uptime, we need to make such validators are removed from
//! the committee, so that the network is as healthy as possible. This is achieved by calculating
//! number of _underperformance_ sessions, which means that number of blocks produced by the
//! validator is less than some predefined threshold.
//! In other words, if a validator:
//! * produced less or equal blocks to a `CurrentEraCommitteeKickOutThresholds::block_count_threshold`, and,
//! * it happened at least `CurrentEraCommitteeKickOutThresholds::underperformed_session_count_threshold` times,
//! then the validator is considered an underperformer and hence removed (ie _kicked out_) from the
//! committee.
//!
//! ## Thresholds
//! There are two kick-out thresholds described above, see [`CommitteeKickOutThresholds`].
//!
//! ### Next era vs current era
//! Current and next era have distinct thresholds values, as we calculate kicks during elections.
//! They follow the same logic as next era committee seats: at the time of planning the first
//! session of next the era, next values become current ones.

#![cfg_attr(not(feature = "std"), no_std)]

mod impls;
mod migrations;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod traits;

use codec::{Decode, Encode};
use frame_support::traits::StorageVersion;
pub use impls::{compute_validator_scaled_total_rewards, LENIENT_THRESHOLD};
pub use pallet::*;
use pallets_support::StorageMigration;
pub use primitives::EraValidators;
use scale_info::TypeInfo;
use sp_std::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    prelude::*,
};

pub type TotalReward = u32;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

#[derive(Decode, Encode, TypeInfo)]
pub struct ValidatorTotalRewards<T>(pub BTreeMap<T, TotalReward>);

#[frame_support::pallet]
pub mod pallet {
    use frame_election_provider_support::{
        ElectionDataProvider, ElectionProvider, Support, Supports,
    };
    use frame_support::{log, pallet_prelude::*, traits::Get};
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use pallet_session::SessionManager;
    use primitives::{BlockCount, CommitteeKickOutThresholds, CommitteeSeats, SessionCount};

    use super::*;
    use crate::traits::{EraInfoProvider, SessionInfoProvider, ValidatorRewardsHandler};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Something that provides information about ongoing eras.
        type EraInfoProvider: EraInfoProvider<AccountId = Self::AccountId>;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Something that provides data for elections.
        type DataProvider: ElectionDataProvider<
            AccountId = Self::AccountId,
            BlockNumber = Self::BlockNumber,
        >;
        /// Nr of blocks in the session.
        #[pallet::constant]
        type SessionPeriod: Get<u32>;
        /// Handler for managing new session.
        type SessionManager: SessionManager<<Self as frame_system::Config>::AccountId>;
        /// Something that provides information about sessions.
        type SessionInfoProvider: SessionInfoProvider<Self>;
        /// Something that handles addition of rewards for validators.
        type ValidatorRewardsHandler: ValidatorRewardsHandler<Self>;

        /// Maximum acceptable kick-out reason length.
        #[pallet::constant]
        type MaximumKickOutReasonLength: Get<u32>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ChangeValidators(Vec<T::AccountId>, Vec<T::AccountId>, CommitteeSeats),

        SetCommitteeKickOutThresholds(CommitteeKickOutThresholds),
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            let on_chain = <Pallet<T> as GetStorageVersion>::on_chain_storage_version();
            T::DbWeight::get().reads(1)
                + match on_chain {
                    _ if on_chain == STORAGE_VERSION => 0,
                    _ if on_chain == StorageVersion::new(0) => {
                        migrations::v0_to_v1::Migration::<T, Self>::migrate()
                            + migrations::v1_to_v2::Migration::<T, Self>::migrate()
                            + migrations::v2_to_v3::Migration::<T, Self>::migrate()
                    }
                    _ if on_chain == StorageVersion::new(1) => {
                        migrations::v1_to_v2::Migration::<T, Self>::migrate()
                            + migrations::v2_to_v3::Migration::<T, Self>::migrate()
                    }
                    _ if on_chain == StorageVersion::new(2) => {
                        migrations::v2_to_v3::Migration::<T, Self>::migrate()
                    }
                    _ => {
                        log::warn!(
                            target: "pallet_elections",
                            "On chain storage version of pallet elections is {:?} but it should not be bigger than 2",
                            on_chain
                        );
                        0
                    }
                }
        }
    }
    /// Desirable size of a committee, see [`CommitteeSeats`].
    #[pallet::storage]
    pub type CommitteeSize<T> = StorageValue<_, CommitteeSeats, ValueQuery>;

    #[pallet::type_value]
    pub fn DefaultNextEraCommitteeSize<T: Config>() -> CommitteeSeats {
        CommitteeSize::<T>::get()
    }

    /// Desired size of a committee in effect from a new era.
    #[pallet::storage]
    pub type NextEraCommitteeSize<T> =
        StorageValue<_, CommitteeSeats, ValueQuery, DefaultNextEraCommitteeSize<T>>;

    /// Next era's list of reserved validators.
    #[pallet::storage]
    pub type NextEraReservedValidators<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// Current era's list of reserved validators.
    #[pallet::storage]
    pub type CurrentEraValidators<T: Config> =
        StorageValue<_, EraValidators<T::AccountId>, ValueQuery>;

    /// Next era's list of non reserved validators.
    #[pallet::storage]
    pub type NextEraNonReservedValidators<T: Config> =
        StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// A lookup how many blocks a validator produced.
    #[pallet::storage]
    pub type SessionValidatorBlockCount<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, BlockCount, ValueQuery>;

    /// Total possible reward per validator for the current era.
    #[pallet::storage]
    pub type ValidatorEraTotalReward<T: Config> =
        StorageValue<_, ValidatorTotalRewards<T::AccountId>, OptionQuery>;

    /// Default value for kick out threshold, see [`CurrentEraCommitteeKickOutThresholds`]
    #[pallet::type_value]
    pub fn DefaultCommitteeKickOutThresholds<T: Config>() -> CommitteeKickOutThresholds {
        CommitteeKickOutThresholds::default()
    }

    /// Configurable threshold values for kick-out functionality, see [`CommitteeKickOutThresholds`]
    #[pallet::storage]
    #[pallet::getter(fn current_era_committee_kick_out_thresholds)]
    pub type CurrentEraCommitteeKickOutThresholds<T> = StorageValue<
        _,
        CommitteeKickOutThresholds,
        ValueQuery,
        DefaultCommitteeKickOutThresholds<T>,
    >;

    /// Next era configurable threshold values for kick-out functionality.
    #[pallet::storage]
    #[pallet::getter(fn next_era_committee_kick_out_thresholds)]
    pub type NextEraCommitteeKickOutThresholds<T> = StorageValue<
        _,
        CommitteeKickOutThresholds,
        ValueQuery,
        DefaultCommitteeKickOutThresholds<T>,
    >;

    /// A lookup for a number of underperformance sessions for a given validator
    #[pallet::storage]
    pub type UnderperformedValidatorSessionCount<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, SessionCount, ValueQuery>;

    /// Validators to be removed from non reserved list in the next era
    #[pallet::storage]
    #[pallet::getter(fn to_be_kicked_out_from_committee)]
    pub type ToBeKickedOutFromCommittee<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<u8, T::MaximumKickOutReasonLength>>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn change_validators(
            origin: OriginFor<T>,
            reserved_validators: Option<Vec<T::AccountId>>,
            non_reserved_validators: Option<Vec<T::AccountId>>,
            committee_size: Option<CommitteeSeats>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let committee_size = committee_size.unwrap_or_else(NextEraCommitteeSize::<T>::get);
            let reserved_validators =
                reserved_validators.unwrap_or_else(NextEraReservedValidators::<T>::get);
            let non_reserved_validators =
                non_reserved_validators.unwrap_or_else(NextEraNonReservedValidators::<T>::get);

            Self::ensure_validators_are_ok(
                reserved_validators.clone(),
                non_reserved_validators.clone(),
                committee_size,
            )?;

            NextEraNonReservedValidators::<T>::put(non_reserved_validators.clone());
            NextEraReservedValidators::<T>::put(reserved_validators.clone());
            NextEraCommitteeSize::<T>::put(committee_size);

            Self::deposit_event(Event::ChangeValidators(
                reserved_validators,
                non_reserved_validators,
                committee_size,
            ));

            Ok(())
        }

        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn set_kick_out_thresholds(
            origin: OriginFor<T>,
            committee_kick_out_thresholds: CommitteeKickOutThresholds,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                committee_kick_out_thresholds.underperformed_session_count_threshold > 0,
                Error::<T>::InvalidKickOutThresholds
            );

            NextEraCommitteeKickOutThresholds::<T>::put(committee_kick_out_thresholds.clone());

            Self::deposit_event(Event::SetCommitteeKickOutThresholds(
                committee_kick_out_thresholds,
            ));

            Ok(())
        }

        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn kick_out_from_committee(
            origin: OriginFor<T>,
            to_be_kicked: T::AccountId,
            kick_out_reason: Vec<u8>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let bounded_description: BoundedVec<u8, T::MaximumKickOutReasonLength> =
                kick_out_reason
                    .try_into()
                    .map_err(|_| Error::<T>::KickOutReasonTooBig)?;

            ToBeKickedOutFromCommittee::<T>::insert(to_be_kicked, bounded_description);

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub non_reserved_validators: Vec<T::AccountId>,
        pub reserved_validators: Vec<T::AccountId>,
        pub committee_seats: CommitteeSeats,
        pub committee_kick_out_thresholds: CommitteeKickOutThresholds,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                non_reserved_validators: Vec::new(),
                reserved_validators: Vec::new(),
                committee_seats: Default::default(),
                committee_kick_out_thresholds: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <CommitteeSize<T>>::put(&self.committee_seats);
            <NextEraCommitteeSize<T>>::put(&self.committee_seats);
            <NextEraNonReservedValidators<T>>::put(&self.non_reserved_validators);
            <NextEraReservedValidators<T>>::put(&self.reserved_validators);
            <CurrentEraValidators<T>>::put(&EraValidators {
                reserved: self.reserved_validators.clone(),
                non_reserved: self.non_reserved_validators.clone(),
            });
            <CurrentEraCommitteeKickOutThresholds<T>>::put(
                &self.committee_kick_out_thresholds.clone(),
            );
            <NextEraCommitteeKickOutThresholds<T>>::put(&self.committee_kick_out_thresholds);
        }
    }

    impl<T: Config> Pallet<T> {
        fn ensure_validators_are_ok(
            reserved_validators: Vec<T::AccountId>,
            non_reserved_validators: Vec<T::AccountId>,
            committee_size: CommitteeSeats,
        ) -> DispatchResult {
            let CommitteeSeats {
                reserved_seats: reserved,
                non_reserved_seats: non_reserved,
            } = committee_size;
            let reserved_len = reserved_validators.len() as u32;
            let non_reserved_len = non_reserved_validators.len() as u32;
            let validators_size = reserved_len + non_reserved_len;

            let committee_size_all = reserved + non_reserved;

            ensure!(
                committee_size_all <= validators_size,
                Error::<T>::NotEnoughValidators
            );

            ensure!(
                reserved <= reserved_len,
                Error::<T>::NotEnoughReservedValidators,
            );

            ensure!(
                non_reserved <= non_reserved_len,
                Error::<T>::NotEnoughReservedValidators,
            );

            let member_set: BTreeSet<_> = reserved_validators
                .into_iter()
                .chain(non_reserved_validators.into_iter())
                .collect();

            ensure!(
                member_set.len() as u32 == validators_size,
                Error::<T>::NonUniqueListOfValidators
            );

            Ok(())
        }

        fn kick_out_underperformed_non_reserved_validators() {
            let to_be_kicked_validators =
                ToBeKickedOutFromCommittee::<T>::iter_keys().collect::<BTreeSet<_>>();
            let non_reserved_validators = NextEraNonReservedValidators::<T>::get()
                .into_iter()
                .collect::<BTreeSet<_>>();
            let filtered_non_reserved_validators = non_reserved_validators
                .difference(&to_be_kicked_validators)
                .cloned()
                .collect::<Vec<_>>();
            NextEraNonReservedValidators::<T>::put(filtered_non_reserved_validators);
            let _result = ToBeKickedOutFromCommittee::<T>::clear(u32::MAX, None);
        }
    }

    #[derive(Debug)]
    pub enum ElectionError {
        DataProvider(&'static str),
    }

    #[pallet::error]
    pub enum Error<T> {
        NotEnoughValidators,
        NotEnoughReservedValidators,
        NotEnoughNonReservedValidators,
        NonUniqueListOfValidators,

        /// underperformed session count threshold must be a positive number, see [`CurrentEraCommitteeKickOutThresholds`]
        InvalidKickOutThresholds,

        /// Kick out reason is too big, ie given vector of bytes is greater than
        /// [`Config::MaximumKickOutReasonLength`]
        KickOutReasonTooBig,
    }

    impl<T: Config> ElectionProvider for Pallet<T> {
        type AccountId = T::AccountId;
        type BlockNumber = T::BlockNumber;
        type Error = ElectionError;
        type DataProvider = T::DataProvider;

        /// The elections are PoA so only the nodes listed in the Validators will be elected as
        /// validators.
        ///
        /// We calculate the supports for them for the sake of eras payouts.
        fn elect() -> Result<Supports<T::AccountId>, Self::Error> {
            Self::kick_out_underperformed_non_reserved_validators();

            let staking_validators = Self::DataProvider::electable_targets(None)
                .map_err(Self::Error::DataProvider)?
                .into_iter()
                .collect::<BTreeSet<_>>();
            let reserved_validators = NextEraReservedValidators::<T>::get()
                .into_iter()
                .collect::<BTreeSet<_>>();
            let non_reserved_validators = NextEraNonReservedValidators::<T>::get()
                .into_iter()
                .collect::<BTreeSet<_>>();

            let eligible_validators =
                &(&reserved_validators | &non_reserved_validators) & &staking_validators;
            let mut supports = eligible_validators
                .into_iter()
                .map(|id| {
                    (
                        id,
                        // Under normal circumstances support will never be `0` since 'self-vote'
                        // is counted in.
                        Support {
                            total: 0,
                            voters: Vec::new(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();

            let voters =
                Self::DataProvider::electing_voters(None).map_err(Self::Error::DataProvider)?;
            for (voter, vote, targets) in voters {
                // The parameter `Staking::MAX_NOMINATIONS` is set to 1 which guarantees that
                // `len(targets) == 1`.
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
