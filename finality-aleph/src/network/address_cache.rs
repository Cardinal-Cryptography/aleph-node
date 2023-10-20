use std::{fmt::Debug, marker::PhantomData, num::NonZeroUsize, sync::Arc};

use lru::LruCache;
use parking_lot::Mutex;
use primitives::{AccountId, AlephSessionApi, AuthorityId, BlockHash, BlockNumber};
use sc_client_api::Backend;
use sp_runtime::traits::{Block, Header};

use crate::{
    abft::NodeIndex,
    session::{SessionBoundaryInfo, SessionId},
    session_map::AuthorityProvider,
    ClientForAleph,
};

pub trait KeyOwnerInfoProvider {
    fn aleph_key_owner(&self, block_number: BlockNumber, key: AuthorityId) -> Option<AccountId>;
}

/// Network details for a given validator in a given session.
#[derive(Debug, Clone)]
pub struct ValidatorAddressingInfo {
    /// Session to which given information applies.
    pub session: SessionId,
    /// Network level address of the validator, i.e. IP address (for validator network)
    pub network_level_address: Option<String>,
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
                NonZeroUsize::try_from(VALIDATOR_ADDRESS_CACHE_SIZE).unwrap(),
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

pub struct ValidatorAddressCacheUpdaterImpl<K: KeyOwnerInfoProvider, A: AuthorityProvider> {
    validator_address_cache: ValidatorAddressCache,
    key_owner_info_provider: K,
    authority_provider: A,
    session_boundary_info: SessionBoundaryInfo,
}

impl<K: KeyOwnerInfoProvider, A: AuthorityProvider> ValidatorAddressCacheUpdaterImpl<K, A> {
    pub fn new(
        validator_address_cache: ValidatorAddressCache,
        key_owner_info_provider: K,
        authority_provider: A,
        session_boundary_info: SessionBoundaryInfo,
    ) -> Self {
        Self {
            validator_address_cache,
            key_owner_info_provider,
            authority_provider,
            session_boundary_info,
        }
    }

    fn owner_at_session(
        &self,
        session: SessionId,
        validator_index: NodeIndex,
    ) -> Option<AccountId> {
        let block_number = self
            .session_boundary_info
            .boundaries_for_session(session)
            .first_block();

        let authority_data = self
            .authority_provider
            .authority_data(block_number.clone())?;
        let aleph_key = authority_data.authorities()[validator_index.0].clone();
        self.key_owner_info_provider
            .aleph_key_owner(block_number, aleph_key)
    }
}

impl<K: KeyOwnerInfoProvider, A: AuthorityProvider> ValidatorAddressCacheUpdater
    for ValidatorAddressCacheUpdaterImpl<K, A>
{
    fn update(&self, validator_index: NodeIndex, info: ValidatorAddressingInfo) {
        if let Some(validator_account) = self.owner_at_session(info.session, validator_index) {
            self.validator_address_cache.insert(validator_account, info);
        }
    }
}

pub struct NoopValidatorAddressCacheUpdater;

impl ValidatorAddressCacheUpdater for NoopValidatorAddressCacheUpdater {
    fn update(&self, _: NodeIndex, _: ValidatorAddressingInfo) {}
}

pub struct KeyOwnerInfoProviderImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B>,
    B: Block<Hash = BlockHash>,
    BE: Backend<B> + 'static,
{
    client: Arc<C>,
    _phantom: PhantomData<(B, BE)>,
}

impl<C, B, BE> KeyOwnerInfoProviderImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B>,
    B: Block<Hash = BlockHash>,
    B::Header: Header<Number = BlockNumber>,
    BE: Backend<B> + 'static,
{
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _phantom: PhantomData,
        }
    }
}

impl<C, B, BE> KeyOwnerInfoProvider for KeyOwnerInfoProviderImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B>,
    B: Block<Hash = BlockHash>,
    B::Header: Header<Number = BlockNumber>,
    BE: Backend<B> + 'static,
{
    fn aleph_key_owner(&self, block_number: BlockNumber, key: AuthorityId) -> Option<AccountId> {
        let block_hash = self.client.block_hash(block_number).ok()??;
        self.client.runtime_api().key_owner(block_hash, key).ok()?
    }
}
