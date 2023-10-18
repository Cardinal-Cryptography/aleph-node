use std::{collections::HashMap, marker::PhantomData, num::NonZeroUsize, sync::Arc};

use lru::LruCache;
use parking_lot::Mutex;
use primitives::{AccountId, AlephSessionApi, AuthorityId, BlockHash, BlockNumber};
use sc_client_api::Backend;
use serde::{Deserialize, Serialize};
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

/// Network details for a given validator. This data is purely informational
/// and can change over time, even within a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorAddressingInfo {
    /// Network level address of the validator, i.e. IP address
    pub network_level_address: String,
    /// PeerId of the validator used in validator (clique) network
    pub validator_network_peer_id: String,
    /// Session to which the given `validator_network_peer_id` corresponds.
    pub session: SessionId,
    /// Vec of substrate's P2P network extra data (e.g. peer_id) that *could* match a given addressing info,
    /// that is, which was sent from the same address as network_level_address.
    /// It can be empty in case we haven't been ever connected directly to a given peer in p2p network,
    /// or even could contain false positives when one validator changed its IP and other node picked it up.
    pub potential_p2p_network_additional_info: Vec<AdditionalP2PAddressingInfo>,
}

/// Additional information about a node in substrate's P2P network. Use only for debugging purposes,
/// as content of this struct rely on unstable substrate's API and can change in future.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalP2PAddressingInfo {
    pub p2p_network_peer_id: String,
    pub version_string: Option<String>,
}

/// Stores most recent information about validator addresses.
#[derive(Debug, Clone)]
pub struct ValidatorAddressCache {
    data: Arc<Mutex<LruCache<AccountId, ValidatorAddressingInfo>>>,
}

const VALIDATOR_ADDRESS_CACHE_SIZE: usize = 300;

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

    pub fn read(&self) -> HashMap<AccountId, ValidatorAddressingInfo> {
        HashMap::from_iter(self.data.lock().iter().map(|(k, v)| (k.clone(), v.clone())))
    }
}

impl Default for ValidatorAddressCache {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ValidatorAddressCacheUpdater {
    /// In session `SessionIndex`, validator `NodeIndex` was using addresses specified in `most_recent_info`.
    /// A session and validator_index identify the validator uniquely.
    fn update(
        &self,
        session: SessionId,
        validator_index: NodeIndex,
        most_recent_info: ValidatorAddressingInfo,
    );
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
}

impl<K: KeyOwnerInfoProvider, A: AuthorityProvider> ValidatorAddressCacheUpdater
    for ValidatorAddressCacheUpdaterImpl<K, A>
{
    fn update(
        &self,
        session: SessionId,
        validator_index: NodeIndex,
        info: ValidatorAddressingInfo,
    ) {
        let block = self
            .session_boundary_info
            .boundaries_for_session(session)
            .first_block();

        if let Some(authority_data) = self.authority_provider.authority_data(block) {
            let aleph_key = authority_data.authorities()[validator_index.0].clone();
            if let Some(validator_stash) = self
                .key_owner_info_provider
                .aleph_key_owner(block, aleph_key)
            {
                self.validator_address_cache.insert(validator_stash, info);
            }
        }
    }
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
        self.client
            .runtime_api()
            .aleph_key_owner(block_hash, key)
            .ok()?
    }
}
