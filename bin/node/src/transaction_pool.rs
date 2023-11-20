use std::sync::Arc;

use futures::StreamExt;
use sc_transaction_pool::{BasicPool, ChainApi};
use sc_transaction_pool_api::{
    error::{Error, IntoPoolError},
    TransactionPool,
};
use sp_api::BlockT;
use sp_runtime::traits;

pub struct TransactionPoolWrapper<A: ChainApi<Block = B>, B: BlockT> {
    pool: Arc<BasicPool<A, B>>,
}

impl<A: ChainApi<Block = B>, B: BlockT> TransactionPoolWrapper<A, B> {
    pub fn new(pool: Arc<BasicPool<A, B>>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl<A: ChainApi<Block = B> + 'static, B: BlockT>
    finality_aleph::metrics::TransactionPoolInfoProvider for TransactionPoolWrapper<A, B>
{
    type TxHash = <<A as ChainApi>::Block as traits::Block>::Hash;
    type Extrinsic = <<A as ChainApi>::Block as traits::Block>::Extrinsic;

    async fn next_transaction(&self) -> Option<Self::TxHash> {
        self.pool.import_notification_stream().next().await
    }

    fn hash_of(&self, extrinsic: &Self::Extrinsic) -> Self::TxHash {
        self.pool.hash_of(extrinsic)
    }

    fn pool_contains(&self, txn: &Self::TxHash) -> bool {
        let knowledge = self.pool.pool().validated_pool().check_is_known(txn, false);
        matches!(
            knowledge.map_err(|e| e.into_pool_error()),
            Err(Ok(Error::AlreadyImported(_)))
        )
    }
}
