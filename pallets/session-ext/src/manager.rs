use frame_support::log::debug;
use pallet_session::SessionManager;
use primitives::EraManager;
use sp_staking::{EraIndex, SessionIndex};
use sp_std::{marker::PhantomData, vec::Vec};

use crate::{
    pallet::{Config, Pallet, SessionValidatorBlockCount},
    traits::EraInfoProvider,
};

struct AlephSessionManager<T: SessionManager<C::AccountId>, E: EraManager, C: Config>(
    PhantomData<(T, E, C)>,
);

impl<T: SessionManager<C::AccountId>, E: EraManager, C: Config> SessionManager<C::AccountId>
    for AlephSessionManager<T, E, C>
{
    fn new_session(new_index: SessionIndex) -> Option<Vec<C::AccountId>> {
        T::new_session(new_index);
        Pallet::<C>::rotate_committee(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        T::end_session(end_index);
        Pallet::<C>::adjust_rewards_for_session();
        Pallet::<C>::calculate_underperforming_validators();
        // clear block count after calculating stats for underperforming validators, as they use
        // SessionValidatorBlockCount for that
        let result = SessionValidatorBlockCount::<C>::clear(u32::MAX, None);
        debug!(target: "pallet_elections", "Result of clearing the `SessionValidatorBlockCount`, {:?}", result.deconstruct());
    }

    fn start_session(start_index: SessionIndex) {
        T::start_session(start_index);
        Pallet::<C>::clear_underperformance_session_counter(start_index);
    }
}
impl<T: SessionManager<C::AccountId>, E: EraManager, C: Config> EraManager
    for AlephSessionManager<T, E, C>
{
    fn on_new_era(era: EraIndex) {
        E::on_new_era(era);
        Pallet::<C>::emit_fresh_bans_event();
    }

    fn new_era_start(era: EraIndex) {
        E::new_era_start(era);
        Pallet::<C>::update_validator_total_rewards(era);
        Pallet::<C>::clear_expired_bans(era);
    }
}

impl<T> pallet_authorship::EventHandler<T::AccountId, T::BlockNumber> for Pallet<T>
where
    T: Config,
{
    fn note_author(validator: T::AccountId) {
        SessionValidatorBlockCount::<T>::mutate(&validator, |count| {
            *count += 1;
        });
    }
}

pub struct SessionManagerExt<E, EM, T, C>(PhantomData<(E, EM, T, C)>)
where
    T: SessionManager<C::AccountId>,
    EM: EraManager,
    E: EraInfoProvider,
    C: Config;

impl<E, EM, T, C> SessionManagerExt<E, EM, T, C>
where
    T: SessionManager<C::AccountId>,
    E: EraInfoProvider,
    EM: EraManager,
    C: Config,
{
    fn session_starts_era(session: SessionIndex, next_era: bool) -> Option<EraIndex> {
        let mut active_era = match E::active_era() {
            Some(ae) => ae,
            // no active era, session can't start it
            _ => return None,
        };
        if next_era {
            active_era += 1;
        }
        if let Some(era_start_index) = E::era_start_session_index(active_era) {
            return if era_start_index == session {
                Some(active_era)
            } else {
                None
            };
        }

        None
    }
}

impl<E, EM, T, C> SessionManager<C::AccountId> for SessionManagerExt<E, EM, T, C>
where
    T: SessionManager<C::AccountId>,
    E: EraInfoProvider,
    EM: EraManager,
    C: Config,
{
    fn new_session(new_index: SessionIndex) -> Option<Vec<C::AccountId>> {
        if let Some(era) = Self::session_starts_era(new_index, true) {
            AlephSessionManager::<T, EM, C>::on_new_era(era);
        }
        AlephSessionManager::<T, EM, C>::new_session(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        AlephSessionManager::<T, EM, C>::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        if let Some(era) = Self::session_starts_era(start_index, false) {
            AlephSessionManager::<T, EM, C>::new_era_start(era)
        }

        AlephSessionManager::<T, EM, C>::start_session(start_index)
    }
}
