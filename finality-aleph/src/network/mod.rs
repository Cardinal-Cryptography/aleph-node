use abft::NodeIndex;
use parity_scale_codec::Codec;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use parking_lot::Mutex;
use sc_client_api::Backend;
use sp_runtime::traits::{Block, Header};

pub mod data;
mod gossip;
#[cfg(test)]
pub mod mock;
pub mod session;
mod substrate;
pub mod tcp;

#[cfg(test)]
pub use gossip::mock::{MockEvent, MockRawNetwork};
pub use gossip::{
    Error as GossipError, Network as GossipNetwork, Protocol, Service as GossipService,
};
use network_clique::{AddressingInformation, NetworkIdentity, PeerId};
use primitives::AlephSessionApi;
use primitives::{AccountId, AuthorityId, BlockHash, BlockNumber};
pub use substrate::{ProtocolNaming, SubstrateNetwork};

use crate::session::{SessionBoundaryInfo, SessionId};
use crate::session_map::AuthorityProvider;
use crate::{abft, BlockId, ClientForAleph};

/// Abstraction for requesting stale blocks.
pub trait RequestBlocks: Clone + Send + Sync + 'static {
    /// Request the given block -- this is supposed to be used only for "old forks".
    fn request_stale_block(&self, block: BlockId);
}

/// A basic alias for properties we expect basic data to satisfy.
pub trait Data: Clone + Codec + Send + Sync + 'static {}

impl<D: Clone + Codec + Send + Sync + 'static> Data for D {}

pub trait KeyOwnerInfoProvider {
    fn aleph_key_owner(&self, block_number: BlockNumber, key: AuthorityId) -> Option<AccountId>;
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

#[derive(Debug, Clone)]
pub struct ValidatorAddressingInfo {
    pub network_level_address: String,
    pub network_level_peer_id: String,
    pub validator_network_peer_id: String,
}

#[derive(Debug, Clone)]
pub struct ValidatorAddressCache {
    data: Arc<Mutex<HashMap<AccountId, ValidatorAddressingInfo>>>,
}

impl ValidatorAddressCache {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, validator_stash: AccountId, info: ValidatorAddressingInfo) {
        self.data.lock().insert(validator_stash, info);
    }

    pub fn as_hashmap(&self) -> HashMap<AccountId, ValidatorAddressingInfo> {
        self.data.lock().clone()
    }
}

pub trait ValidatorAddressCacheUpdater {
    /// In session `SessionIndex`, validator `NodeIndex` was using addresses specified in `info`.
    fn update(&self, session: SessionId, validator_index: NodeIndex, info: ValidatorAddressingInfo);
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
