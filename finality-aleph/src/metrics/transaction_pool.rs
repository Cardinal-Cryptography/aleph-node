use std::sync::Arc;

use futures::StreamExt;
use sc_transaction_pool::{BasicPool, ChainApi};
use sc_transaction_pool_api::{
    error::{Error, IntoPoolError},
    ImportNotificationStream, TransactionPool,
};
use sp_api::BlockT;
use sp_runtime::traits;
use sp_runtime::traits::Member;

#[async_trait::async_trait]
pub trait TransactionPoolInfoProvider {
    type TxHash: Member + std::hash::Hash;
    type Extrinsic;
    async fn next_transaction(&mut self) -> Option<Self::TxHash>;

    fn hash_of(&self, extrinsic: &Self::Extrinsic) -> Self::TxHash;

    fn pool_contains(&self, transaction: &Self::TxHash) -> bool;
}

type HashFor<A> = <<A as ChainApi>::Block as traits::Block>::Hash;

pub struct TransactionPoolWrapper<A: ChainApi<Block = B> + 'static, B: BlockT> {
    pool: Arc<BasicPool<A, B>>,
    import_notification_stream: ImportNotificationStream<HashFor<A>>,
}

impl<A: ChainApi<Block = B>, B: BlockT> TransactionPoolWrapper<A, B> {
    pub fn new(pool: Arc<BasicPool<A, B>>) -> Self {
        Self {
            pool: pool.clone(),
            import_notification_stream: pool.import_notification_stream(),
        }
    }
}

#[async_trait::async_trait]
impl<A: ChainApi<Block = B> + 'static, B: BlockT> crate::metrics::TransactionPoolInfoProvider
    for TransactionPoolWrapper<A, B>
{
    type TxHash = HashFor<A>;
    type Extrinsic = <<A as ChainApi>::Block as traits::Block>::Extrinsic;

    async fn next_transaction(&mut self) -> Option<Self::TxHash> {
        self.import_notification_stream.next().await
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

#[cfg(test)]
pub mod test {
    use std::{sync::Arc, time::Duration};

    use futures::{future, StreamExt};
    use sc_basic_authorship::ProposerFactory;
    use sc_block_builder::BlockBuilderProvider;
    use sc_client_api::{BlockchainEvents, HeaderBackend};
    use sc_transaction_pool::{BasicPool, FullChainApi};
    use sc_transaction_pool_api::{MaintainedTransactionPool, TransactionPool};
    use sp_api::BlockT;
    use sp_consensus::{BlockOrigin, DisableProofRecording, Environment, Proposer as _};
    use sp_runtime::transaction_validity::TransactionSource;
    use substrate_test_runtime::{Extrinsic, ExtrinsicBuilder, Transfer};
    use substrate_test_runtime_client::{AccountKeyring, ClientBlockImportExt, ClientExt};

    use crate::{
        metrics::{transaction_pool::TransactionPoolWrapper, TransactionPoolInfoProvider},
        testing::mocks::{
            TBlock, THash, TestBackend, TestClient, TestClientBuilder, TestClientBuilderExt,
        },
    };

    type TChainApi = FullChainApi<TestClient, TBlock>;
    type FullTransactionPool = BasicPool<TChainApi, TBlock>;
    type TProposerFactory =
        ProposerFactory<FullTransactionPool, TestBackend, TestClient, DisableProofRecording>;

    pub struct TestTransactionPoolSetup {
        pub client: Arc<TestClient>,
        pub pool: Arc<FullTransactionPool>,
        pub proposer_factory: TProposerFactory,
        pub transaction_pool_info_provider: TransactionPoolWrapper<TChainApi, TBlock>,
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

        pub fn xt(
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
    async fn test_pool_contains() {
        let client = Arc::new(TestClientBuilder::new().build());
        let mut setup = TestTransactionPoolSetup::new(client.clone());
        let genesis = client.genesis_hash();
        let xt = setup.xt(AccountKeyring::Alice, AccountKeyring::Bob, 0);

        assert!(!setup
            .transaction_pool_info_provider
            .pool_contains(&setup.pool.hash_of(&xt)));
        setup.submit(&genesis, xt.clone()).await;
        assert!(setup
            .transaction_pool_info_provider
            .pool_contains(&setup.pool.hash_of(&xt)));

        let block_1 = setup.propose_block(genesis, None).await;

        assert_eq!(block_1.extrinsics.len(), 1);
        assert!(!setup
            .transaction_pool_info_provider
            .pool_contains(&setup.pool.hash_of(&xt)));
    }

    #[tokio::test]
    async fn test_pool_contains_for_invalid_transactions() {
        let client = Arc::new(TestClientBuilder::new().build());
        let mut setup = TestTransactionPoolSetup::new(client.clone());
        let genesis = client.genesis_hash();

        let xt1 = setup.xt(AccountKeyring::Alice, AccountKeyring::Bob, 0);
        let xt2 = setup.xt(AccountKeyring::Alice, AccountKeyring::Charlie, 0);

        setup.submit(&genesis, xt1.clone()).await;
        assert!(setup
            .transaction_pool_info_provider
            .pool_contains(&setup.pool.hash_of(&xt1)));

        // external block, including xt2 with the same nonce as xt1
        let mut block_1_builder = client
            .new_block_at(genesis, Default::default(), false)
            .unwrap();
        block_1_builder.push(xt2).unwrap();
        let block_1 = block_1_builder.build().unwrap().block;
        setup.import_block(block_1).await;

        // pool maintenance after importing should remove xt1
        assert!(!setup
            .transaction_pool_info_provider
            .pool_contains(&setup.pool.hash_of(&xt1)));
    }
}
