use primitives::{CommitteeSeats, EraValidators, SessionValidators};
use sp_staking::EraIndex;
use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

use crate::{
    traits::ValidatorProvider, CommitteeSize, Config, CurrentEraValidators, NextEraCommitteeSize,
    NextEraNonReservedValidators, NextEraReservedValidators, Pallet,
};

impl<T> Pallet<T>
where
    T: Config,
{
    fn populate_next_era_validators_on_next_era_start(era: EraIndex) {
        let elected_committee = BTreeSet::from_iter(T::ValidatorProvider::elected_validators(era));

        let retain_elected = |vals: Vec<T::AccountId>| -> Vec<T::AccountId> {
            vals.into_iter()
                .filter(|v| elected_committee.contains(v))
                .collect()
        };

        let reserved_validators = NextEraReservedValidators::<T>::get();
        let non_reserved_validators = NextEraNonReservedValidators::<T>::get();
        let committee_size = NextEraCommitteeSize::<T>::get();

        CurrentEraValidators::<T>::put(EraValidators {
            reserved: retain_elected(reserved_validators),
            non_reserved: retain_elected(non_reserved_validators),
        });
        CommitteeSize::<T>::put(committee_size);
    }
}

impl<T: Config> primitives::EraManager for Pallet<T> {
    fn on_new_era(era: EraIndex) {
        Self::populate_next_era_validators_on_next_era_start(era);
    }
    fn new_era_start(_: EraIndex) {}
}

impl<T: Config> primitives::BanHandler for Pallet<T> {
    type AccountId = T::AccountId;
    fn can_ban(account_id: &Self::AccountId) -> bool {
        !NextEraReservedValidators::<T>::get().contains(account_id)
    }
}

impl<T: Config + pallet_staking::Config + pallet_session::Config> primitives::ValidatorProvider
    for Pallet<T>
where
    T::ValidatorId: Into<T::AccountId>,
{
    type AccountId = T::AccountId;
    fn current_era_validators() -> Option<EraValidators<Self::AccountId>> {
        if pallet_staking::ActiveEra::<T>::get().map(|ae| ae.index) == Some(0) {
            return None;
        }
        Some(CurrentEraValidators::<T>::get())
    }
    fn current_era_committee_size() -> Option<CommitteeSeats> {
        if pallet_staking::ActiveEra::<T>::get().map(|ae| ae.index) == Some(0) {
            return None;
        }
        Some(CommitteeSize::<T>::get())
    }
    fn current_session_committee_and_non_committee() -> SessionValidators<Self::AccountId> {
        let committee: BTreeSet<T::AccountId> = pallet_session::Validators::<T>::get()
            .into_iter()
            .map(|a| a.into())
            .collect();
        let EraValidators {
            reserved,
            non_reserved,
        } = CurrentEraValidators::<T>::get();

        let non_committee = non_reserved
            .into_iter()
            .chain(reserved.into_iter())
            .filter(|a| !committee.contains(a))
            .collect();

        SessionValidators {
            committee: committee.into_iter().collect(),
            non_committee,
        }
    }
}
