use std::{marker::PhantomData, sync::Arc};

use ::primitives::{AlephSessionApi, Version};
use jsonrpsee::{
    core::{Error, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use sp_api::{BlockId, BlockT, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;

#[rpc(client, server)]
pub trait FinalityVersionApi<BlockHash> {
    #[method(name = "finalityVersion_nextSessionFinalityVersion")]
    fn next_session_finality_version(&self, at: Option<BlockHash>) -> RpcResult<Version>;
}

pub struct FinalityVersion<C, M> {
    client: Arc<C>,
    _marker: PhantomData<M>,
}

impl<C, M> FinalityVersion<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> FinalityVersionApiServer<<Block as BlockT>::Hash> for FinalityVersion<C, Block>
    where
        Block: BlockT,
        C: Send + Sync + 'static,
        C: ProvideRuntimeApi<Block>,
        C: HeaderBackend<Block>,
        C::Api: AlephSessionApi<Block>,
{
    fn next_session_finality_version(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Version> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.next_session_finality_version(&at);
        runtime_api_result.map_err(|e| {
            Error::Call(CallError::Custom(ErrorObject::owned(
                1001, // arbitrary value
                "Unable to obtain finality version for the next session!",
                Some(format!("{:?}", e)),
            )))
        })
    }
}
