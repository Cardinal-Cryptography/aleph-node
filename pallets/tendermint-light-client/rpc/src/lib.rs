// pub use self::gen_client::Client as TransactionPaymentClient;
use codec::{Codec, Decode};
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
// pub use pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi as TransactionPaymentRuntimeApi;
// use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, InclusionFee, RuntimeDispatchInfo};
use pallet_tendermint_light_client_rpc_runtime_api::LightBlockStorage;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::sync::Arc;

#[rpc]
pub trait TendermintLightClientApi // <BlockHash, ResponseType>
{
    #[rpc(name = "tendermintLightClient_getLastImportedBlock")]
    fn get_last_imported_block(&self) -> Result<Option<LightBlockStorage>>;
}

/// A struct that implements the [`TransactionPaymentApi`].
pub struct TendermintLightClient;

impl TendermintLightClient {
    pub fn new() -> Self {
        Self {}
    }
}

impl TendermintLightClientApi for TendermintLightClient {
    fn get_last_imported_block(&self) -> Result<Option<LightBlockStorage>> {
        todo!()
    }
}
