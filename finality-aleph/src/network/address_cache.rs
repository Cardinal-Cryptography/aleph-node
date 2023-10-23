use std::{fmt::Debug, marker::PhantomData, num::NonZeroUsize, sync::Arc};

use lru::LruCache;
use parking_lot::Mutex;
use primitives::{AccountId, AlephSessionApi, BlockHash, BlockNumber};
use sc_client_api::Backend;
use sp_runtime::traits::{Block, Header};

use crate::{
    abft::NodeIndex,
    session::{SessionBoundaryInfo, SessionId},
    session_map::{AuthorityProvider, AuthorityProviderImpl},
    ClientForAleph,
};

pub trait ValidatorIndexToAccountIdConverter {
    fn account(&self, session: SessionId, validator_index: NodeIndex) -> Option<AccountId>;
}

/// Network details for a given validator in a given session.
#[derive(Debug, Clone)]
pub struct ValidatorAddressingInfo {
    /// Session to which given information applies.
    pub session: SessionId,
    /// Network level address of the validator, i.e. IP address (for validator network)
    pub network_level_address: String,
    /// PeerId of the validator used in validator (clique) network
    pub validator_network_peer_id: String,
}

/// Stores most recent information about validator addresses.
#[derive(Debug, Clone)]
pub struct ValidatorAddressCache {
    data: Arc<Mutex<LruCache<AccountId, ValidatorAddressingInfo>>>,
}

const VALIDATOR_ADDRESS_CACHE_SIZE: usize = 1000;

impl ValidatorAddressCache {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::try_from(VALIDATOR_ADDRESS_CACHE_SIZE)
                    .expect("the cache size is a non-zero constant"),
            ))),
        }
    }

    pub fn insert(&self, validator_stash: AccountId, info: ValidatorAddressingInfo) {
        self.data.lock().put(validator_stash, info);
    }
}

impl Default for ValidatorAddressCache {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ValidatorAddressCacheUpdater {
    /// In session `session_info.session`, validator `NodeIndex` was using addresses specified in
    /// `session_info`. A session and validator_index identify the validator uniquely.
    fn update(&self, validator_index: NodeIndex, session_info: ValidatorAddressingInfo);
}

enum ValidatorAddressCacheUpdaterImpl<C: ValidatorIndexToAccountIdConverter> {
    Noop,
    BackendBased {
        validator_address_cache: ValidatorAddressCache,
        key_owner_info_provider: C,
    },
}

/// Construct a struct that can be used to update `validator_address_cache`, if it is `Some`.
/// If passed None, the returned struct will be a no-op.
pub fn validator_address_cache_updater<C: ValidatorIndexToAccountIdConverter>(
    validator_address_cache: Option<ValidatorAddressCache>,
    key_owner_info_provider: C,
) -> impl ValidatorAddressCacheUpdater {
    match validator_address_cache {
        Some(validator_address_cache) => ValidatorAddressCacheUpdaterImpl::BackendBased {
            validator_address_cache,
            key_owner_info_provider,
        },
        None => ValidatorAddressCacheUpdaterImpl::Noop,
    }
}

impl<C: ValidatorIndexToAccountIdConverter> ValidatorAddressCacheUpdater
    for ValidatorAddressCacheUpdaterImpl<C>
{
    fn update(&self, validator_index: NodeIndex, info: ValidatorAddressingInfo) {
        if let ValidatorAddressCacheUpdaterImpl::BackendBased {
            validator_address_cache,
            key_owner_info_provider,
        } = self
        {
            if let Some(validator) = key_owner_info_provider.account(info.session, validator_index)
            {
                validator_address_cache.insert(validator, info)
            }
        }
    }
}

pub struct ValidatorIndexToAccountIdConverterImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B>,
    B: Block<Hash = BlockHash>,
    BE: Backend<B> + 'static,
{
    client: Arc<C>,
    session_boundary_info: SessionBoundaryInfo,
    authority_provider: AuthorityProviderImpl<C, B, BE>,
    _phantom: PhantomData<(B, BE)>,
}

impl<C, B, BE> ValidatorIndexToAccountIdConverterImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B>,
    B: Block<Hash = BlockHash>,
    B::Header: Header<Number = BlockNumber>,
    BE: Backend<B> + 'static,
{
    pub fn new(client: Arc<C>, session_boundary_info: SessionBoundaryInfo) -> Self {
        Self {
            client: client.clone(),
            session_boundary_info,
            authority_provider: AuthorityProviderImpl::new(client),
            _phantom: PhantomData,
        }
    }
}

impl<C, B, BE> ValidatorIndexToAccountIdConverter
    for ValidatorIndexToAccountIdConverterImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B>,
    B: Block<Hash = BlockHash>,
    B::Header: Header<Number = BlockNumber>,
    BE: Backend<B> + 'static,
{
    fn account(&self, session: SessionId, validator_index: NodeIndex) -> Option<AccountId> {
        let block_number = self
            .session_boundary_info
            .boundaries_for_session(session)
            .first_block();
        let block_hash = self.client.block_hash(block_number).ok()??;

        let authority_data = self.authority_provider.authority_data(block_number)?;
        let aleph_key = authority_data.authorities()[validator_index.0].clone();
        self.client
            .runtime_api()
            .key_owner(block_hash, aleph_key)
            .ok()?
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    struct MockConverter;
    impl ValidatorIndexToAccountIdConverter for MockConverter {
        fn account(&self, _: SessionId, _: NodeIndex) -> Option<AccountId> {
            None
        }
    }
    pub fn noop_updater() -> impl ValidatorAddressCacheUpdater {
        validator_address_cache_updater(None, MockConverter)
    }
}
