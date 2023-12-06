use std::{num::NonZeroUsize, time::Instant};

use futures::{future, Stream, StreamExt};
use log::warn;
use lru::LruCache;
use sc_client_api::{
    BlockBackend, BlockImportNotification, FinalityNotification, FinalityNotifications,
    ImportNotifications,
};
use sp_api::{BlockT, HeaderT};
use sp_blockchain::{lowest_common_ancestor, HeaderMetadata};
use sp_runtime::{
    traits::{SaturatedConversion, Zero},
    Saturating,
};
use substrate_prometheus_endpoint::{
    register, Gauge, Histogram, HistogramOpts, PrometheusError, Registry, U64,
};
use tokio::select;

use crate::{
    metrics::{exponential_buckets_two_sided, TransactionPoolInfoProvider, LOG_TARGET},
    BlockNumber,
};

// Size of transaction cache: 32B (Hash) + 16B (Instant) * `100_000` is approximately 4.8MB
const TRANSACTION_CACHE_SIZE: usize = 100_000;
// Maximum number of transactions to recheck if they are still in the pool, per single loop iteration.
const MAX_RECHECKED_TRANSACTIONS: usize = 4;
const BUCKETS_FACTOR: f64 = 1.4;

enum ChainStateMetrics {
    Prometheus {
        top_finalized_block: Gauge<U64>,
        best_block: Gauge<U64>,
        reorgs: Histogram,
        time_till_block_inclusion: Histogram,
    },
    Noop,
}

impl ChainStateMetrics {
    fn new(registry: Option<Registry>) -> Result<Self, PrometheusError> {
        let registry = match registry {
            Some(registry) => registry,
            None => return Ok(ChainStateMetrics::Noop),
        };

        Ok(ChainStateMetrics::Prometheus {
            top_finalized_block: register(
                Gauge::new("aleph_top_finalized_block", "no help")?,
                &registry,
            )?,
            best_block: register(Gauge::new("aleph_best_block", "no help")?, &registry)?,
            reorgs: register(
                Histogram::with_opts(
                    HistogramOpts::new("aleph_reorgs", "Number of reorgs by length")
                        .buckets(vec![1., 2., 3., 5., 10.]),
                )?,
                &registry,
            )?,
            time_till_block_inclusion: register(
                Histogram::with_opts(
                    HistogramOpts::new("aleph_transaction_to_block_time", "no help")
                        .buckets(exponential_buckets_two_sided(2000.0, BUCKETS_FACTOR, 2, 8)?),
                )?,
                &registry,
            )?,
        })
    }

    fn noop() -> Self {
        ChainStateMetrics::Noop
    }

    fn update_best_block(&self, number: BlockNumber) {
        if let ChainStateMetrics::Prometheus { best_block, .. } = self {
            best_block.set(number as u64)
        }
    }

    fn update_top_finalized_block(&self, number: BlockNumber) {
        if let ChainStateMetrics::Prometheus {
            top_finalized_block,
            ..
        } = self
        {
            top_finalized_block.set(number as u64);
        }
    }

    fn report_reorg(&self, length: BlockNumber) {
        if let ChainStateMetrics::Prometheus { reorgs, .. } = self {
            reorgs.observe(length as f64);
        }
    }

    fn report_transaction_in_block(&self, elapsed: std::time::Duration) {
        if let ChainStateMetrics::Prometheus {
            time_till_block_inclusion,
            ..
        } = self
        {
            time_till_block_inclusion.observe(elapsed.as_secs_f64() * 1000.);
        }
    }
}

pub async fn run_chain_state_metrics<
    XT,
    HE: HeaderT<Number = BlockNumber, Hash = B::Hash>,
    B: BlockT<Header = HE, Extrinsic = XT>,
    BE: HeaderMetadata<B> + BlockBackend<B>,
    TP: TransactionPoolInfoProvider<Extrinsic = XT>,
>(
    backend: &BE,
    import_notifications: ImportNotifications<B>,
    finality_notifications: FinalityNotifications<B>,
    registry: Option<Registry>,
    mut transaction_pool_info_provider: TP,
) {
    let metrics = match ChainStateMetrics::new(registry) {
        Ok(metrics) => metrics,
        Err(e) => {
            warn!(target: LOG_TARGET, "Failed to create metrics: {e}.");
            ChainStateMetrics::noop()
        }
    };

    let mut cache: LruCache<TP::TxHash, Instant> = LruCache::new(
        NonZeroUsize::new(TRANSACTION_CACHE_SIZE).expect("the cache size is a non-zero constant"),
    );

    let mut best_block_notifications = import_notifications
        .fuse()
        .filter(|notification| future::ready(notification.is_new_best));
    let mut finality_notifications = finality_notifications.fuse();
    let mut previous_best: Option<HE> = None;

    loop {
        iteration(
            backend,
            &mut best_block_notifications,
            &mut finality_notifications,
            &metrics,
            &mut transaction_pool_info_provider,
            &mut cache,
            &mut previous_best,
        )
        .await;
    }
}

async fn iteration<
    XT,
    HE: HeaderT<Number = BlockNumber, Hash = B::Hash>,
    B: BlockT<Header = HE, Extrinsic = XT>,
    BE: HeaderMetadata<B> + BlockBackend<B>,
    TP: TransactionPoolInfoProvider<Extrinsic = XT>,
    BBN: Stream<Item = BlockImportNotification<B>> + Unpin,
    FN: Stream<Item = FinalityNotification<B>> + Unpin,
>(
    backend: &BE,
    best_block_notifications: &mut BBN,
    finality_notifications: &mut FN,
    metrics: &ChainStateMetrics,
    transaction_pool_info_provider: &mut TP,
    cache: &mut LruCache<TP::TxHash, Instant>,
    previous_best: &mut Option<HE>,
) {
    select! {
        maybe_block = best_block_notifications.next() => {
            match maybe_block {
                Some(block) => {
                    let number = (*block.header.number()).saturated_into::<BlockNumber>();
                    metrics.update_best_block(number);
                    if let Some(reorg_len) = detect_reorgs(backend, previous_best.clone(), block.header.clone()) {
                        metrics.report_reorg(reorg_len);
                    }
                    if let Ok(Some(body)) = backend.block_body(block.hash) {
                        report_extrinsics_included_in_block(transaction_pool_info_provider, &body, metrics, cache);
                    }
                    *previous_best = Some(block.header);
                }
                None => {
                    warn!(target: LOG_TARGET, "Import notification stream ended unexpectedly");
                }
            }
        },
        maybe_block = finality_notifications.next() => {
            match maybe_block {
                Some(block) => {
                    // Sometimes finalization can also cause best block update. However,
                    // RPC best block subscription won't notify about that immediately, so
                    // we also don't update there. Also in that case, substrate sets best_block to
                    // the newly finalized block (see test), so the best block will be updated
                    // after importing anything on the newly finalized branch.
                    metrics.update_top_finalized_block(*block.header.number());
                }
                None => {
                    warn!(target: LOG_TARGET, "Finality notification stream ended unexpectedly");
                }
            }
        },
        maybe_transaction = transaction_pool_info_provider.next_transaction() => {
            match maybe_transaction {
                Some(hash) => {
                    // Putting new transaction can evict the oldest one. However, even if the
                    // removed transaction was actually still in the pool, we don't have
                    // any guarantees that it could be included in the block. Therefore, we
                    // we ignore such transaction.
                    cache.put(hash, Instant::now());
                }
                None => {
                    warn!(target: LOG_TARGET, "Transaction stream ended unexpectedly");
                }
            }
        },
    }

    let mut rechecked_transactions = 0;
    while let Some((hash, instant)) = cache.pop_lru() {
        if !transaction_pool_info_provider.pool_contains(&hash) {
            cache.pop_lru();
        } else {
            cache.put(hash, instant);
        }
        rechecked_transactions += 1;
        if rechecked_transactions > MAX_RECHECKED_TRANSACTIONS {
            break;
        }
    }
}

fn detect_reorgs<HE: HeaderT<Hash = B::Hash>, B: BlockT<Header = HE>, BE: HeaderMetadata<B>>(
    backend: &BE,
    prev_best: Option<HE>,
    best: HE,
) -> Option<HE::Number> {
    let prev_best = prev_best?;
    if best.hash() == prev_best.hash() || *best.parent_hash() == prev_best.hash() {
        // Quit early when no change or the best is a child of the previous best.
        return None;
    }
    let lca = lowest_common_ancestor(backend, best.hash(), prev_best.hash()).ok()?;
    let len = prev_best
        .number()
        .saturating_sub(lca.number)
        .saturated_into::<HE::Number>();
    if len == HE::Number::zero() {
        return None;
    }
    Some(len)
}

fn report_extrinsics_included_in_block<
    'a,
    TP: TransactionPoolInfoProvider,
    I: IntoIterator<Item = &'a TP::Extrinsic>,
>(
    pool: &'a TP,
    body: I,
    metrics: &ChainStateMetrics,
    cache: &mut LruCache<TP::TxHash, Instant>,
) where
    <TP as TransactionPoolInfoProvider>::TxHash: std::hash::Hash + PartialEq + Eq,
{
    for xt in body {
        let hash = pool.hash_of(xt);
        if let Some(insert_time) = cache.pop(&hash) {
            let elapsed = insert_time.elapsed();
            metrics.report_transaction_in_block(elapsed);
        }
    }
}

#[cfg(test)]
mod test {
    use futures::{FutureExt, StreamExt};
    use parity_scale_codec::Encode;
    use sc_basic_authorship::ProposerFactory;
    use sc_block_builder::BlockBuilderProvider;
    use sc_client_api::{BlockchainEvents, HeaderBackend};
    use sc_transaction_pool::{BasicPool, FullChainApi};
    use sc_transaction_pool_api::{ChainEvent, MaintainedTransactionPool, TransactionPool};
    use sp_api::BlockT;
    use sp_blockchain::tree_route;
    use sp_consensus::{BlockOrigin, DisableProofRecording, Environment, Proposer as _};
    use sp_runtime::transaction_validity::TransactionSource;
    use std::{sync::Arc, time::Duration};
    use substrate_prometheus_endpoint::Registry;
    use substrate_test_runtime::{Extrinsic, ExtrinsicBuilder, Transfer};
    use substrate_test_runtime_client::{AccountKeyring, ClientBlockImportExt, ClientExt};

    use super::*;
    use crate::{
        metrics::transaction_pool::TransactionPoolWrapper,
        testing::{
            client_chain_builder::ClientChainBuilder,
            mocks::{
                TBlock, THash, TestBackend, TestClient, TestClientBuilder, TestClientBuilderExt,
            },
        },
    };

    #[tokio::test]
    async fn when_finalizing_with_reorg_best_block_is_set_to_that_finalized_block() {
        let client = Arc::new(TestClientBuilder::new().build());
        let client_builder = Arc::new(TestClientBuilder::new().build());
        let mut chain_builder = ClientChainBuilder::new(client.clone(), client_builder);

        let chain_a = chain_builder
            .build_and_import_branch_above(&chain_builder.genesis_hash(), 5)
            .await;

        // (G) - A1 - A2 - A3 - A4 - A5;

        assert_eq!(
            client.chain_info().finalized_hash,
            chain_builder.genesis_hash()
        );
        assert_eq!(client.chain_info().best_number, 5);

        let chain_b = chain_builder
            .build_and_import_branch_above(&chain_a[0].header.hash(), 3)
            .await;
        chain_builder.finalize_block(&chain_b[0].header.hash());

        // (G) - (A1) - A2 - A3 - A4 - A5
        //         \
        //          (B2) - B3 - B4

        assert_eq!(client.chain_info().best_number, 2);
        assert_eq!(client.chain_info().finalized_hash, chain_b[0].header.hash());
    }

    #[tokio::test]
    async fn test_reorg_detection() {
        let client = Arc::new(TestClientBuilder::new().build());
        let client_builder = Arc::new(TestClientBuilder::new().build());
        let mut chain_builder = ClientChainBuilder::new(client.clone(), client_builder);

        let a = chain_builder
            .build_and_import_branch_above(&chain_builder.genesis_hash(), 5)
            .await;
        let b = chain_builder
            .build_and_import_branch_above(&a[0].header.hash(), 3)
            .await;
        let c = chain_builder
            .build_and_import_branch_above(&a[2].header.hash(), 2)
            .await;

        //                  - C0 - C1
        //                /
        // G - A0 - A1 - A2 - A3 - A4
        //      \
        //        - B0 - B1 - B2

        for (prev, new_best, expected) in [
            (&a[1], &a[2], None),
            (&a[1], &a[4], None),
            (&a[1], &a[1], None),
            (&a[2], &b[0], Some(2)),
            (&b[0], &a[2], Some(1)),
            (&c[1], &b[2], Some(4)),
        ] {
            assert_eq!(
                detect_reorgs(
                    client.as_ref(),
                    Some(prev.header().clone()),
                    new_best.header().clone()
                ),
                expected,
            );
        }
    }

    type TChainApi = FullChainApi<TestClient, TBlock>;
    type FullTransactionPool = BasicPool<TChainApi, TBlock>;
    type TProposerFactory =
        ProposerFactory<FullTransactionPool, TestBackend, TestClient, DisableProofRecording>;

    struct TestSetup {
        pub client: Arc<TestClient>,
        pub pool: Arc<FullTransactionPool>,
        pub proposer_factory: TProposerFactory,
        pub transaction_pool_info_provider: TransactionPoolWrapper<TChainApi, TBlock>,
        pub metrics: ChainStateMetrics,
        pub cache: LruCache<THash, Instant>,
        pub best_block_notifications:
            Box<dyn Stream<Item = BlockImportNotification<TBlock>> + Unpin>,
        pub finality_notifications: Box<dyn Stream<Item = FinalityNotification<TBlock>> + Unpin>,
    }

    impl TestSetup {
        fn new() -> Self {
            let client = Arc::new(TestClientBuilder::new().build());
            let spawner = sp_core::testing::TaskExecutor::new();
            let pool = BasicPool::new_full(
                Default::default(),
                true.into(),
                None,
                spawner.clone(),
                client.clone(),
            );

            let proposer_factory =
                ProposerFactory::new(spawner, client.clone(), pool.clone(), None, None);

            let transaction_pool_info_provider = TransactionPoolWrapper::new(pool.clone());
            let registry = Registry::new();
            let metrics = ChainStateMetrics::new(Some(registry)).expect("metrics");
            let cache = LruCache::new(NonZeroUsize::new(10).expect("cache"));

            let best_block_notifications = Box::new(
                client
                    .every_import_notification_stream()
                    .fuse()
                    .filter(|notification| future::ready(notification.is_new_best)),
            );

            let finality_notifications = Box::new(client.finality_notification_stream().fuse());

            TestSetup {
                client,
                pool,
                proposer_factory,
                transaction_pool_info_provider,
                metrics,
                cache,
                best_block_notifications,
                finality_notifications,
            }
        }

        async fn run_one_metrics_loop_iteration(&mut self) {
            iteration(
                self.client.as_ref(),
                &mut self.best_block_notifications,
                &mut self.finality_notifications,
                &self.metrics,
                &mut self.transaction_pool_info_provider,
                &mut self.cache,
                &mut None,
            )
            .await
        }

        fn iter_while_possible(&mut self) -> u32 {
            let mut res = 0;
            while self
                .run_one_metrics_loop_iteration()
                .now_or_never()
                .is_some()
            {
                res += 1;
            }
            res
        }

        async fn new_block(&mut self, at: THash, weight_limit: Option<usize>) -> TBlock {
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
                .unwrap();

            let block = block.block;
            self.client
                .import(BlockOrigin::NetworkBroadcast, block.clone())
                .await
                .unwrap();

            block
        }

        fn extrinsic(
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

        async fn submit(&mut self, at: &THash, xt: Extrinsic) {
            self.pool
                .submit_one(*at, TransactionSource::External, xt)
                .await
                .unwrap();
        }

        fn transactions_histogram(&self) -> &Histogram {
            match &self.metrics {
                ChainStateMetrics::Prometheus {
                    time_till_block_inclusion,
                    ..
                } => time_till_block_inclusion,
                _ => panic!("metrics"),
            }
        }
    }

    #[tokio::test]
    async fn transactions_get_reported() {
        let mut setup = TestSetup::new();
        let genesis = setup.client.info().genesis_hash;
        let xt = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 0);

        let time_before_submit = Instant::now();
        setup.submit(&genesis, xt).await;

        assert_eq!(
            setup.iter_while_possible(),
            1,
            "'In pool' notification wasn't sent"
        );
        let time_after_submit = Instant::now();

        tokio::time::sleep(Duration::from_millis(20)).await;

        let time_before_import = Instant::now();
        let _block_1 = setup.new_block(genesis, None).await;
        let pre_count = setup.transactions_histogram().get_sample_count();

        assert_eq!(
            setup.iter_while_possible(),
            1,
            "Block import notification wasn't sent"
        );

        let time_after_import = Instant::now();

        let duration =
            Duration::from_secs_f64(setup.transactions_histogram().get_sample_sum() / 1000.);
        let eps = Duration::from_nanos(1);

        assert_eq!(pre_count, 0);
        assert_eq!(setup.transactions_histogram().get_sample_count(), 1);
        assert!(duration >= time_before_import - time_after_submit - eps);
        assert!(duration <= time_after_import - time_before_submit + eps);
    }

    #[tokio::test]
    async fn transactions_are_reported_only_if_ready_when_added_to_the_pool() {
        let mut setup = TestSetup::new();
        let genesis = setup.client.info().genesis_hash;

        let xt0 = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 0);
        let xt1 = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 1);
        let xt2 = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 2);

        setup.submit(&genesis, xt1.clone()).await;

        // No notification for tx1 as it is not ready
        assert_eq!(
            setup.iter_while_possible(),
            0,
            "Future transactions should not be reported"
        );

        setup.submit(&genesis, xt0.clone()).await;
        setup.submit(&genesis, xt2.clone()).await;

        // Both notification for tx0 and tx2
        assert_eq!(setup.iter_while_possible(), 2);

        let block_1 = setup.new_block(genesis, None).await;
        // Block import notification. Tx1 notification never appears
        assert_eq!(setup.iter_while_possible(), 1);
        // All 3 extrinsics are included in a block
        assert_eq!(block_1.extrinsics.len(), 3);
    }
    #[tokio::test]
    async fn retracted_transactions_are_reported_once() {
        let mut setup = TestSetup::new();
        let genesis = setup.client.info().genesis_hash;

        let xt1 = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 0);
        let xt2 = setup.extrinsic(AccountKeyring::Charlie, AccountKeyring::Dave, 0);

        setup.submit(&genesis, xt1.clone()).await;
        setup.submit(&genesis, xt2.clone()).await;

        // make sure import notifications are received before block import
        assert_eq!(setup.iter_while_possible(), 2);

        let block_1a = setup.new_block(genesis, None).await;
        setup
            .pool
            .maintain(ChainEvent::NewBestBlock {
                hash: block_1a.hash(),
                tree_route: None,
            })
            .await;

        assert_eq!(block_1a.extrinsics.len(), 2);
        assert_eq!(setup.iter_while_possible(), 1);
        assert_eq!(setup.transactions_histogram().get_sample_count(), 2);

        let sum_before = setup.transactions_histogram().get_sample_sum();

        // external fork block with xt1
        let mut block_1b_builder = setup
            .client
            .new_block_at(genesis, Default::default(), false)
            .unwrap();
        block_1b_builder.push(xt1).unwrap();

        let block_1b = block_1b_builder.build().unwrap().block;
        setup
            .client
            .import(BlockOrigin::NetworkBroadcast, block_1b.clone())
            .await
            .unwrap();
        setup
            .pool
            .maintain(ChainEvent::NewBestBlock {
                hash: block_1b.hash(),
                tree_route: Some(Arc::new(
                    tree_route(setup.client.as_ref(), block_1a.hash(), genesis).unwrap(),
                )),
            })
            .await;
        setup.client.finalize_block(block_1b.hash(), None).unwrap();
        setup
            .pool
            .maintain(ChainEvent::Finalized {
                hash: block_1b.hash(),
                tree_route: Arc::new([genesis]),
            })
            .await;

        let block_2b = setup.new_block(block_1b.hash(), None).await;
        setup
            .pool
            .maintain(ChainEvent::NewBestBlock {
                hash: block_2b.hash(),
                tree_route: None,
            })
            .await;

        assert_eq!(block_2b.extrinsics.len(), 1);
        assert_eq!(setup.transactions_histogram().get_sample_count(), 2);
        assert_eq!(setup.transactions_histogram().get_sample_sum(), sum_before);
    }

    #[tokio::test]
    async fn transactions_discarded_in_block_authorship_are_not_reported_at_that_time() {
        let mut setup = TestSetup::new();
        let genesis = setup.client.info().genesis_hash;

        let xt1 = setup.extrinsic(AccountKeyring::Alice, AccountKeyring::Bob, 0);
        let xt2 = setup.extrinsic(AccountKeyring::Charlie, AccountKeyring::Dave, 0);

        setup.submit(&genesis, xt1.clone()).await;
        setup.submit(&genesis, xt2.clone()).await;
        assert_eq!(setup.iter_while_possible(), 2);

        let time_after_submit = Instant::now();

        let block_1 = setup
            .new_block(genesis, Some(2 * xt1.encoded_size() - 1))
            .await;

        assert_eq!(setup.iter_while_possible(), 1);
        assert_eq!(block_1.extrinsics.len(), 1);
        assert_eq!(setup.transactions_histogram().get_sample_count(), 1);
        let sample_1 = setup.transactions_histogram().get_sample_sum();

        tokio::time::sleep(Duration::from_millis(10)).await;

        let time_before_block_2 = Instant::now();
        let block_2 = setup
            .new_block(block_1.hash(), Some(2 * xt1.encoded_size() - 1))
            .await;
        let iters_block_2 = setup.iter_while_possible();

        assert_eq!(iters_block_2, 1);
        assert_eq!(block_2.extrinsics.len(), 1);
        assert_eq!(setup.transactions_histogram().get_sample_count(), 2);

        let sample_2 = setup.transactions_histogram().get_sample_sum() - sample_1;

        let duration = Duration::from_secs_f64(sample_2 / 1000.0);
        let eps = Duration::from_nanos(1);

        assert!(duration >= time_before_block_2 - time_after_submit - eps);
    }
}
