use crate::{Config, Pallet};

impl<T> pallet_session::SessionManager<T::AccountId> for Pallet<T>
where
    T: Config,
{
    fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <T as Config>::SessionManager::new_session(new_index);
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        Self::new_session()
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
where T: Config,
{
    // Check if a schedule version change has moved into the past. If so, update history.
    // Does not reset the scheduled version.
    fn update_version_change_history() {
        let current_session = Self::current_session();

        if let Some(previously_scheduled_version_change) =
        <AlephBFTScheduledVersionChange<T>>::get()
        {
            let previously_scheduled_session = previously_scheduled_version_change.session;
            let previously_scheduled_version =
                previously_scheduled_version_change.version_incoming;

            if previously_scheduled_session <= current_session {
                // Record the previously scheduled version in version change history.
                <AlephBFTVersion<T>>::set(
                    previously_scheduled_session,
                    previously_scheduled_version,
                );
                Self::deposit_event(Event::UpdateAlephBFTVersionHistory(
                    previously_scheduled_version_change,
                ));
            }
        }
    }
}