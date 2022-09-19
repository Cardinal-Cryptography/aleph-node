use primitives::SessionIndex;
use sp_std::vec::Vec;

use crate::{AlephBFTScheduledVersionChange, AlephBFTVersion, Config, Event, Pallet};

impl<T> pallet_session::SessionManager<T::AccountId> for Pallet<T>
where
    T: Config,
{
    fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <T as Config>::SessionManager::new_session(new_index)
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <T as Config>::SessionManager::new_session_genesis(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        <T as Config>::SessionManager::end_session(end_index);
    }

    fn start_session(start_index: SessionIndex) {
        <T as Config>::SessionManager::start_session(start_index);
        Self::update_version_change_history();
    }
}

impl<T> Pallet<T>
where
    T: Config,
{
    // Check if a schedule version change has moved into the past. Update history, even if there is
    // no change. Does not reset the scheduled version.
    fn update_version_change_history() {
        let current_session = Self::current_session();

        // Carry over version from previous session.
        if current_session != 0 {
            if let Ok(version_for_previous_session) =
                <AlephBFTVersion<T>>::try_get(current_session - 1)
            {
                <AlephBFTVersion<T>>::set(current_session, version_for_previous_session);
            }
        }

        if let Some(scheduled_version_change) = <AlephBFTScheduledVersionChange<T>>::get() {
            let scheduled_session = scheduled_version_change.session;
            let scheduled_version = scheduled_version_change.version_incoming;

            // Record the scheduled version in version change history as it moves into the past.
            if scheduled_session == current_session {
                <AlephBFTVersion<T>>::set(current_session, scheduled_version);

                Self::deposit_event(Event::ReachedScheduledAlephBFTVersionChange(
                    scheduled_version_change,
                ));
            }
        }
    }
}
