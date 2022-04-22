use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use pallet_tendermint_light_client_rpc_runtime_api::LightBlockStorage;
pub use pallet_tendermint_light_client_rpc_runtime_api::TendermintLightClientApi as TendermintLightClientRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait TendermintLightClientApi<BlockHash> {
    #[rpc(name = "tendermintLightClient_getLastImportedBlock")]
    fn get_last_imported_block(&self, at: Option<BlockHash>) -> Result<Option<LightBlockStorage>>;
}

/// A struct that implements the [`TransactionPaymentApi`].
pub struct TendermintLightClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> TendermintLightClient<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> TendermintLightClientApi<<Block as BlockT>::Hash> for TendermintLightClient<C, Block>
where
    Block: BlockT,
    C: 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: TendermintLightClientRuntimeApi<Block>,
{
    fn get_last_imported_block(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<LightBlockStorage>> {
        let api = self.client.runtime_api();

        let at = BlockId::hash(at.unwrap_or_else(||
			                         // If the block hash is not supplied assume the last finalized block
			                         self.client.info().best_hash));

        api.get_last_imported_block(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(666), // there is just one error possible here, runtime error
            message: "Unable to query last imported block.".into(),
            data: Some(e.to_string().into()),
        })
    }
}
