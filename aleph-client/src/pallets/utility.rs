use subxt::ext::sp_core::H256;

use crate::{api, Call, SignedConnection, TxStatus};

#[async_trait::async_trait]
pub trait UtilityApi {
    async fn batch_call(&self, calls: Vec<Call>, status: TxStatus) -> anyhow::Result<H256>;
}

#[async_trait::async_trait]
impl UtilityApi for SignedConnection {
    async fn batch_call(&self, calls: Vec<Call>, status: TxStatus) -> anyhow::Result<H256> {
        let tx = api::tx().utility().batch(calls);

        self.send_tx(tx, status).await
    }
}
