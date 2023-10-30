use std::{
    fmt::{Display, Formatter},
    marker::PhantomData,
    sync::Arc,
};

use aleph_runtime::SessionKeys;
use parity_scale_codec::Decode;
use sc_client_api::Backend;
use sc_consensus_aura::AuraApi;
use sp_application_crypto::key_types::AURA;
use sp_core::twox_128;
use sp_runtime::traits::{Block, OpaqueKeys};

use crate::{
    aleph_primitives::{AccountId, AuraId},
    BlockHash, ClientForAleph,
};

/// Trait handling connection between host code and runtime storage
pub trait RuntimeApi {
    type Error: Display;
    fn next_aura_authorities(&self, at: BlockHash) -> Result<Vec<AuraId>, Self::Error>;
}

type QueuedKeys = Vec<(AccountId, SessionKeys)>;

#[derive(Clone)]
pub struct RuntimeApiImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B> + AuraApi<B, AuraId>,
    B: Block<Hash = BlockHash>,
    BE: Backend<B> + 'static,
{
    client: Arc<C>,
    _phantom: PhantomData<(B, BE)>,
}

impl<C, B, BE> RuntimeApiImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B> + AuraApi<B, AuraId>,
    B: Block<Hash = BlockHash>,
    BE: Backend<B> + 'static,
{
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _phantom: PhantomData,
        }
    }

    fn read_storage<D: Decode>(&self, pallet: &str, item: &str, at_block: BlockHash) -> Option<D> {
        let storage_key = [twox_128(pallet.as_bytes()), twox_128(item.as_bytes())].concat();

        let encoded = match self
            .client
            .storage(at_block, &sc_client_api::StorageKey(storage_key))
        {
            Ok(Some(e)) => e,
            _ => return None,
        };

        D::decode(&mut encoded.0.as_ref()).ok()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ApiError {
    NoStorage,
}

impl Display for ApiError {
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        todo!()
    }
}

impl<C, B, BE> RuntimeApi for RuntimeApiImpl<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: crate::aleph_primitives::AlephSessionApi<B> + AuraApi<B, AuraId>,
    B: Block<Hash = BlockHash>,
    BE: Backend<B> + 'static,
{
    type Error = ApiError;

    fn next_aura_authorities(&self, at: BlockHash) -> Result<Vec<AuraId>, Self::Error> {
        let queued_keys: QueuedKeys = match self.read_storage("Session", "QueuedKeys", at) {
            Some(qk) => qk,
            None => return Err(ApiError::NoStorage),
        };

        Ok(queued_keys
            .into_iter()
            .filter_map(|(_, keys)| keys.get(AURA))
            .collect())
    }
}
