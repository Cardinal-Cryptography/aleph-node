use std::sync::Arc;

use futures::StreamExt;
use sc_transaction_pool_api::{ImportNotificationStream, TransactionFor, TransactionPool};
use sp_runtime::traits::Member;

#[async_trait::async_trait]
pub trait TransactionPoolInfoProvider {
    type TxHash: Member + std::hash::Hash;
    type Extrinsic: sp_runtime::traits::Extrinsic;
    async fn next_transaction(&mut self) -> Option<Self::TxHash>;

    fn hash_of(&self, extrinsic: &Self::Extrinsic) -> Self::TxHash;
}

pub struct TransactionPoolWrapper<T: TransactionPool> {
    pool: Arc<T>,
    import_notification_stream: ImportNotificationStream<T::Hash>,
}

impl<T: TransactionPool> TransactionPoolWrapper<T> {
    pub fn new(pool: Arc<T>) -> Self {
        Self {
            pool: pool.clone(),
            import_notification_stream: pool.import_notification_stream(),
        }
    }
}

#[async_trait::async_trait]
impl<T: TransactionPool> TransactionPoolInfoProvider for TransactionPoolWrapper<T> {
    type TxHash = T::Hash;
    type Extrinsic = TransactionFor<T>;

    async fn next_transaction(&mut self) -> Option<Self::TxHash> {
        self.import_notification_stream.next().await
    }

    fn hash_of(&self, extrinsic: &Self::Extrinsic) -> Self::TxHash {
        self.pool.hash_of(extrinsic)
    }
}

#[cfg(test)]
pub mod test {
    use std::{sync::Arc, time::Duration};

    use futures::{future, FutureExt, StreamExt};
    use sc_basic_authorship::ProposerFactory;
    use sc_client_api::{BlockchainEvents, HeaderBackend};
    use sc_transaction_pool::{BasicPool, FullChainApi};
    use sc_transaction_pool_api::{MaintainedTransactionPool, TransactionPool, TransactionStatus};
    use sp_api::BlockT;
    use sp_consensus::{BlockOrigin, DisableProofRecording, Environment, Proposer as _};
    use sp_runtime::transaction_validity::TransactionSource;
    use substrate_test_runtime::{Extrinsic, ExtrinsicBuilder, Transfer};
    use substrate_test_runtime_client::{AccountKeyring, ClientBlockImportExt, ClientExt};

    use crate::{
        metrics::transaction_pool::TransactionPoolWrapper,
        testing::mocks::{
            Backend, TBlock, THash, TestClient, TestClientBuilder, TestClientBuilderExt,
        },
    };

    type TChainApi = FullChainApi<TestClient, TBlock>;
    type FullTransactionPool = BasicPool<TChainApi, TBlock>;
    type TProposerFactory =
        ProposerFactory<FullTransactionPool, Backend, TestClient, DisableProofRecording>;

    pub struct TestTransactionPoolSetup {
        pub client: Arc<TestClient>,
        pub pool: Arc<FullTransactionPool>,
        pub proposer_factory: TProposerFactory,
        pub transaction_pool_info_provider: TransactionPoolWrapper<BasicPool<TChainApi, TBlock>>,
    }

    impl TestTransactionPoolSetup {
        pub fn new(client: Arc<TestClient>) -> Self {
            let spawner = sp_core::testing::TaskExecutor::new();
            let pool = BasicPool::new_full(
                Default::default(),
                true.into(),
                None,
                spawner.clone(),
                client.clone(),
            );
            let transaction_pool_info_provider = TransactionPoolWrapper::new(pool.clone());

            let proposer_factory =
                ProposerFactory::new(spawner, client.clone(), pool.clone(), None, None);

            TestTransactionPoolSetup {
                client,
                pool,
                proposer_factory,
                transaction_pool_info_provider,
            }
        }

        pub async fn propose_block(&mut self, at: THash, weight_limit: Option<usize>) -> TBlock {
            let proposer = self
                .proposer_factory
                .init(&self.client.expect_header(at).unwrap())
                .await
                .unwrap();

            let block = proposer
                .propose(
                    Default::default(),
                    Default::default(),
                    Duration::from_secs(30),
                    weight_limit,
                )
                .await
                .unwrap()
                .block;

            self.import_block(block).await
        }

        pub async fn import_block(&mut self, block: TBlock) -> TBlock {
            let stream = self.client.every_import_notification_stream();
            self.client
                .import(BlockOrigin::Own, block.clone())
                .await
                .unwrap();

            let notification = stream
                .filter(|notification| future::ready(notification.hash == block.hash()))
                .next()
                .await
                .expect("Notification was sent");

            if notification.is_new_best {
                self.pool.maintain(notification.try_into().unwrap()).await;
            }

            block
        }

        pub async fn finalize(&mut self, hash: THash) {
            let stream = self.client.finality_notification_stream();
            self.client.finalize_block(hash, None).unwrap();
            let notification = stream
                .filter(|notification| future::ready(notification.hash == hash))
                .next()
                .await
                .expect("Notification was sent");

            self.pool.maintain(notification.into()).await;
        }

        pub fn extrinsic(
            &self,
            sender: AccountKeyring,
            receiver: AccountKeyring,
            nonce: u64,
        ) -> Extrinsic {
            let tx = Transfer {
                amount: Default::default(),
                nonce,
                from: sender.into(),
                to: receiver.into(),
            };
            ExtrinsicBuilder::new_transfer(tx).build()
        }

        pub async fn submit(&mut self, at: &THash, xt: Extrinsic) {
            self.pool
                .submit_one(*at, TransactionSource::External, xt)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    pub async fn in_block_notification_fired_only_for_best_blocks() {
        // Test assuring that substrate's implementation details hasn't changed
        let client = Arc::new(TestClientBuilder::new().build());
        let mut setup = TestTransactionPoolSetup::new(client.clone());
        let block1 = setup.propose_block(client.genesis_hash(), None).await;
        let block2 = setup.propose_block(block1.hash(), None).await;

        let xt = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 0);
        let mut watcher = setup
            .pool
            .submit_and_watch(client.genesis_hash(), TransactionSource::External, xt)
            .await
            .unwrap();

        assert_eq!(
            watcher.next().now_or_never().expect("status was updated"),
            Some(TransactionStatus::Ready),
        );

        let block_1b = setup.propose_block(client.genesis_hash(), None).await;
        // Transaction was added, but no notification was fired
        assert_eq!(block_1b.extrinsics.len(), 1);
        assert_eq!(watcher.next().now_or_never(), None);

        let block_3 = setup.propose_block(block2.hash(), None).await;
        // Now transaction was also added, and notification was fired
        assert_eq!(block_1b.extrinsics.len(), 1);
        assert_eq!(
            watcher.next().now_or_never().expect("status was updated"),
            Some(TransactionStatus::InBlock((block_3.hash(), 0)))
        );
    }
}
