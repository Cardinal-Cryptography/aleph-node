use crate::{api, BlockHash, Call, SignedConnection, TxStatus};

/// Pallet utility api.
#[async_trait::async_trait]
pub trait UtilityApi {
    /// API for [`batch`](https://paritytech.github.io/substrate/master/pallet_utility/pallet/struct.Pallet.html#method.batch) call.
    async fn batch_call(&self, calls: Vec<Call>, status: TxStatus) -> anyhow::Result<BlockHash>;
}

#[async_trait::async_trait]
impl UtilityApi for SignedConnection {
    async fn batch_call(&self, calls: Vec<Call>, status: TxStatus) -> anyhow::Result<BlockHash> {
        let tx = api::tx().utility().batch(calls);

        self.send_tx(tx, status).await
    }
}
