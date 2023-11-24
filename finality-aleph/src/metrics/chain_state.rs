use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use futures::{future, StreamExt};
use log::warn;
use sc_client_api::{FinalityNotifications, ImportNotifications};
use sp_api::{BlockT, HeaderT};
use sp_blockchain::{lowest_common_ancestor, HeaderMetadata};
use sp_consensus::BlockOrigin;
use sp_runtime::traits::{SaturatedConversion, Zero};
use substrate_prometheus_endpoint::{
    register, Counter, Gauge, Histogram, HistogramOpts, PrometheusError, Registry, U64,
};
use tokio::select;

use crate::{metrics::LOG_TARGET, BlockNumber};

trait ChainStateMeasure {
    fn increase_own_hopeless_blocks(&self, value: u64);
    fn update_best_block(&self, number: BlockNumber);
    fn update_top_finalized_block(&self, number: BlockNumber);
    fn report_reorg(&self, length: BlockNumber);
}

enum ChainStateMetrics {
    Prometheus {
        own_finalized_blocks: Counter<U64>,
        own_hopeless_blocks: Counter<U64>,
        top_finalized_block: Gauge<U64>,
        best_block: Gauge<U64>,
        reorgs: Histogram,
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
            own_finalized_blocks: register(
                Counter::new("aleph_own_finalized_blocks", "no help")?,
                &registry,
            )?,
            own_hopeless_blocks: register(
                Counter::new("aleph_own_hopeless_blocks", "no help")?,
                &registry,
            )?,
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
        })
    }

    fn noop() -> Self {
        ChainStateMetrics::Noop
    }
}

impl ChainStateMeasure for ChainStateMetrics {
    fn increase_own_hopeless_blocks(&self, value: u64) {
        if let ChainStateMetrics::Prometheus {
            own_hopeless_blocks,
            ..
        } = self
        {
            own_hopeless_blocks.inc_by(value);
        }
    }

    fn update_best_block(&self, number: BlockNumber) {
        if let ChainStateMetrics::Prometheus { best_block, .. } = self {
            best_block.set(number as u64)
        }
    }

    fn update_top_finalized_block(&self, number: BlockNumber) {
        if let ChainStateMetrics::Prometheus {
            top_finalized_block,
            own_finalized_blocks,
            ..
        } = self
        {
            top_finalized_block.set(number as u64);
            own_finalized_blocks.inc();
        }
    }

    fn report_reorg(&self, length: BlockNumber) {
        if let ChainStateMetrics::Prometheus { reorgs, .. } = self {
            reorgs.observe(length as f64);
        }
    }
}

pub struct ChainStateMetricsRunner<HE, B, BE>
where
    HE: HeaderT<Number = BlockNumber, Hash = B::Hash>,
    B: BlockT<Header = HE>,
    BE: HeaderMetadata<B>,
{
    metrics: Box<dyn ChainStateMeasure + Send>,
    waiting_for_finality: HashMap<HE::Number, Vec<HE::Hash>>,
    waiting_for_import: HashSet<HE::Hash>,
    _phantom: PhantomData<(HE, B, BE)>,
}

impl<HE, B, BE> ChainStateMetricsRunner<HE, B, BE>
where
    HE: HeaderT<Number = BlockNumber, Hash = B::Hash>,
    B: BlockT<Header = HE>,
    BE: HeaderMetadata<B>,
{
    pub fn new(registry: Option<Registry>) -> Self {
        ChainStateMetricsRunner {
            metrics: Box::new(match ChainStateMetrics::new(registry) {
                Ok(metrics) => metrics,
                Err(e) => {
                    warn!(target: LOG_TARGET, "Failed to create metrics: {e}.");
                    ChainStateMetrics::noop()
                }
            }),
            waiting_for_finality: HashMap::new(),
            waiting_for_import: HashSet::new(),
            _phantom: PhantomData,
        }
    }

    pub async fn run_chain_state_metrics(
        mut self,
        backend: &BE,
        import_notifications: ImportNotifications<B>,
        finality_notifications: FinalityNotifications<B>,
    ) {
        let mut interesting_block_notifications =
            import_notifications.fuse().filter(|notification| {
                future::ready(notification.is_new_best || notification.origin == BlockOrigin::Own)
            });
        let mut finality_notifications = finality_notifications.fuse();
        let mut previous_best: Option<HE> = None;
        let mut finality_level = 0;

        loop {
            select! {
                maybe_block = interesting_block_notifications.next() => {
                    match maybe_block {
                        Some(block) => {
                            if block.origin == BlockOrigin::Own && !self.waiting_for_import.remove(&block.header.hash()) {
                                if *block.header.number() <= finality_level {
                                    self.metrics.increase_own_hopeless_blocks(1);
                                }
                                else {
                                    match self.waiting_for_finality.get_mut(block.header.number()) {
                                        Some(hashes) => hashes.push(block.header.hash()),
                                        None => {
                                            self.waiting_for_finality.insert(*block.header.number(), vec![block.header.hash()]);
                                        },
                                    }
                                }
                            }
                            if block.is_new_best {
                                let number = (*block.header.number()).saturated_into::<BlockNumber>();
                                self.metrics.update_best_block(number);
                                if let Some(reorg_len) = Self::detect_reorgs(backend, previous_best, block.header.clone()) {
                                    self.metrics.report_reorg(reorg_len);
                                }
                                previous_best = Some(block.header);
                            }
                        }
                        None => {
                            warn!(target: LOG_TARGET, "Import notification stream ended unexpectedly");
                            break;
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
                            self.metrics.update_top_finalized_block(*block.header.number());
                            finality_level = *block.header.number();
                            if let Some(hashes) = self.waiting_for_finality.remove(block.header.number()) {
                                self.metrics.increase_own_hopeless_blocks(hashes.iter().filter(|h| **h != block.header.hash()).count() as u64);
                            }
                        }
                        None => {
                            warn!(target: LOG_TARGET, "Finality notification stream ended unexpectedly");
                            break;
                        }
                    }
                },
            }
        }
    }

    fn detect_reorgs(backend: &BE, prev_best: Option<HE>, best: HE) -> Option<HE::Number> {
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
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, sync::Arc};

    use futures::channel::mpsc::{unbounded, UnboundedSender};
    use sc_client_api::BlockchainEvents;
    use sp_api::BlockT;

    use super::*;
    use crate::{
        metrics::ChainStateMetricsRunner,
        testing::{
            client_chain_builder::ClientChainBuilder,
            mocks::{TestClientBuilder, TestClientBuilderExt},
        },
    };

    struct MockMetrics {
        completed_tx: UnboundedSender<()>,
        finalized_count: RefCell<usize>,
        hopeless_count: RefCell<usize>,
        best_count: RefCell<usize>,
        expected_finalized: usize,
        expected_hopeless: usize,
        expected_best: usize,
    }

    impl MockMetrics {
        fn new(ef: usize, eh: usize, eb: usize, tx: UnboundedSender<()>) -> Self {
            MockMetrics {
                completed_tx: tx,
                finalized_count: 0.into(),
                hopeless_count: 0.into(),
                best_count: 0.into(),
                expected_finalized: ef,
                expected_hopeless: eh,
                expected_best: eb,
            }
        }

        fn try_finish(&self) {
            if *self.finalized_count.borrow() == self.expected_finalized
                && *self.hopeless_count.borrow() == self.expected_hopeless
                && *self.best_count.borrow() == self.expected_best
            {
                let _ = self.completed_tx.unbounded_send(());
            }
        }
    }

    impl ChainStateMeasure for MockMetrics {
        fn increase_own_hopeless_blocks(&self, value: u64) {
            *self.hopeless_count.borrow_mut() += value as usize;
            self.try_finish();
        }

        fn update_best_block(&self, _number: BlockNumber) {
            *self.best_count.borrow_mut() += 1;
            self.try_finish();
        }

        fn update_top_finalized_block(&self, _number: BlockNumber) {
            *self.finalized_count.borrow_mut() += 1;
            self.try_finish();
        }

        fn report_reorg(&self, _length: BlockNumber) {}
    }

    impl<HE, B, BE> ChainStateMetricsRunner<HE, B, BE>
    where
        HE: HeaderT<Number = BlockNumber, Hash = B::Hash>,
        B: BlockT<Header = HE>,
        BE: HeaderMetadata<B>,
    {
        fn from_mock_metrics(metrics: MockMetrics) -> ChainStateMetricsRunner<HE, B, BE> {
            ChainStateMetricsRunner {
                metrics: Box::new(metrics),
                waiting_for_finality: HashMap::new(),
                waiting_for_import: HashSet::new(),
                _phantom: PhantomData,
            }
        }
    }

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
                ChainStateMetricsRunner::detect_reorgs(
                    client.as_ref(),
                    Some(prev.header().clone()),
                    new_best.header().clone()
                ),
                expected,
            );
        }
    }

    #[tokio::test]
    async fn random_test() {
        let client = Arc::new(TestClientBuilder::new().build());
        let client_builder = Arc::new(TestClientBuilder::new().build());
        let mut chain_builder = ClientChainBuilder::new(client.clone(), client_builder);

        let import_stream = chain_builder.client.import_notification_stream();
        let finality_stream = chain_builder.client.finality_notification_stream();

        let a = chain_builder
            .build_and_import_branch_above(&chain_builder.genesis_hash(), 5)
            .await;
        chain_builder
            .build_and_import_branch_above(&a[0].header.hash(), 3)
            .await;
        chain_builder
            .build_and_import_branch_above(&a[2].header.hash(), 2)
            .await;

        // Finalize branch A and expect B and C to become hopeless
        //                  - C0 - C1
        //                /
        // G - A0 - A1 - A2 - A3 - A4
        //      \
        //        - B0 - B1 - B2

        for block in a {
            chain_builder.finalize_block(&block.header().hash());
        }

        let (finish_tx, mut finish_rx) = unbounded();
        let metrics = MockMetrics::new(5, 5, 5, finish_tx);

        tokio::spawn(async move {
            let chain_state_metrics_runner = ChainStateMetricsRunner::from_mock_metrics(metrics);
            chain_state_metrics_runner
                .run_chain_state_metrics(client.as_ref(), import_stream, finality_stream)
                .await;
        });

        assert_ne!(finish_rx.next().await, None);
    }
}
