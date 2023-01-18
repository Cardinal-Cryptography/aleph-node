use primitives::{AuthorityId, SessionIndex};
use crate::Config;
use frame_support::{
    log,
    sp_runtime::{RuntimeAppPublic, traits::OpaqueKeys},
};

use sp_std::prelude::*;

/// Information provider from `pallet_session`. Loose pallet coupling via traits.
pub trait SessionInfoProvider {
    fn current_session() -> SessionIndex;
}

/// Provider . Used only as default value in case of missing this information in our pallet. This can
/// happen after for the session after runtime upgrade and older ones.
pub trait NextSessionAuthorityProvider<T: Config> {
    fn next_authorities() -> Vec<T::AuthorityId>;
}

impl<T> SessionInfoProvider for pallet_session::Pallet<T>
where
    T: pallet_session::Config,
{
    fn current_session() -> SessionIndex {
        pallet_session::CurrentIndex::<T>::get()
    }
}

impl<T> NextSessionAuthorityProvider<T> for pallet_session::Pallet<T>
where T: Config + pallet_session::Config {
    fn next_authorities() -> Vec<T::AuthorityId> {
        let next: Option<Vec<_>> = pallet_session::Pallet::<T>::queued_keys().iter()
            .map(|(_, key)| key.get(AuthorityId::ID))
            .collect();

        next.unwrap_or_else(|| {
            log::error!(target: "pallet_aleph", "Missing next session keys");
            vec![]
        })
    }
}