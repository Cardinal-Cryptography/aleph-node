use frame_support::{pallet_prelude::Get, traits::Currency};
use sp_std::vec::Vec;

pub type SessionId = u32;
pub type EraId = u32;

pub trait SessionInfoProvider<T: frame_system::Config> {
    fn current_session() -> SessionId;
    fn current_committee() -> Vec<T::AccountId>;
}

impl<T> SessionInfoProvider<T> for pallet_session::Pallet<T>
where
    T: pallet_session::Config,
    T::ValidatorId: Into<T::AccountId>,
{
    fn current_session() -> SessionId {
        pallet_session::Pallet::<T>::current_index()
    }

    fn current_committee() -> Vec<T::AccountId> {
        pallet_session::Validators::<T>::get()
            .into_iter()
            .map(|a| a.into())
            .collect()
    }
}

pub trait ValidatorRewardsHandler<T: frame_system::Config> {
    fn all_era_validators(era: EraId) -> Vec<T::AccountId>;
    fn validator_totals(era: EraId) -> Vec<(T::AccountId, u128)>;
    fn add_rewards(rewards: impl IntoIterator<Item = (T::AccountId, u32)>);
}

impl<T> ValidatorRewardsHandler<T> for pallet_staking::Pallet<T>
where
    T: pallet_staking::Config,
    <T::Currency as Currency<T::AccountId>>::Balance: Into<u128>,
{
    fn all_era_validators(era: EraId) -> Vec<T::AccountId> {
        pallet_staking::ErasStakers::<T>::iter_key_prefix(era).collect()
    }

    fn validator_totals(era: EraId) -> Vec<(T::AccountId, u128)> {
        pallet_staking::ErasStakers::<T>::iter_prefix(era)
            .map(|(validator, exposure)| (validator, exposure.total.into()))
            .collect()
    }

    fn add_rewards(rewards: impl IntoIterator<Item = (T::AccountId, u32)>) {
        pallet_staking::Pallet::<T>::reward_by_ids(rewards);
    }
}

pub trait EraInfoProvider {
    fn current_era() -> Option<EraId>;
    fn era_start(era: EraId) -> Option<SessionId>;
    fn sessions_per_era() -> u32;
}

impl<T> EraInfoProvider for pallet_staking::Pallet<T>
where
    T: pallet_staking::Config,
{
    fn current_era() -> Option<EraId> {
        pallet_staking::ActiveEra::<T>::get().map(|ae| ae.index)
    }

    fn era_start(era: EraId) -> Option<SessionId> {
        pallet_staking::ErasStartSessionIndex::<T>::get(era)
    }

    fn sessions_per_era() -> u32 {
        T::SessionsPerEra::get()
    }
}
