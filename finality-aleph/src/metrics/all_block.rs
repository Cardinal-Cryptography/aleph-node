use log::warn;
use substrate_prometheus_endpoint::Registry;

use super::{finality_rate::FinalityRateMetrics, timing::DefaultClock, Checkpoint};
use crate::aleph_primitives::Header;
use crate::metrics::best_block_related_metrics::{BestBlockRelatedMetrics, ReorgDetector};
use crate::metrics::transaction_pool::TransactionPoolMetrics;
use crate::{metrics::LOG_TARGET, BlockId, TimingBlockMetrics};

pub struct SloMetrics<TxHash, ReorgDetector> {
    all_block_metrics: AllBlockMetrics,
    transaction_metrics: TransactionPoolMetrics<TxHash>,
    best_block_related_metrics: BestBlockRelatedMetrics<ReorgDetector>,
}

impl<TxHash: std::hash::Hash + Eq, R: ReorgDetector> SloMetrics<TxHash, R> {
    pub fn new(registry: Option<&Registry>, reorg_detector: R) -> Self {
        let all_block_metrics = AllBlockMetrics::new(registry);
        let transaction_metrics = TransactionPoolMetrics::new(registry).unwrap_or_else(|e| {
            warn!(
                target: LOG_TARGET,
                "Failed to register Prometheus transaction pool metrics: {:?}.", e
            );
            TransactionPoolMetrics::Noop
        });
        let best_block_related_metrics =
            BestBlockRelatedMetrics::new(registry.cloned(), reorg_detector).unwrap_or_else(|e| {
                warn!(
                    target: LOG_TARGET,
                    "Failed to register Prometheus best block related metrics: {:?}.", e
                );
                BestBlockRelatedMetrics::Noop
            });

        SloMetrics {
            all_block_metrics,
            transaction_metrics,
            best_block_related_metrics,
        }
    }

    pub fn report_best_block_imported(&self, header: Header) {
        self.best_block_related_metrics
            .report_best_block_imported(header);
    }

    pub fn report_block_finalized(&self, block_id: BlockId) {
        self.best_block_related_metrics
            .report_block_finalized(block_id);
    }

    pub fn report_block(&self, block_id: BlockId, checkpoint: Checkpoint, own: bool) {
        self.all_block_metrics
            .report_block(block_id, checkpoint, Some(own));
    }
}

#[derive(Clone)]
pub struct AllBlockMetrics {
    timing_metrics: TimingBlockMetrics<DefaultClock>,
    finality_rate_metrics: FinalityRateMetrics,
}

impl AllBlockMetrics {
    pub fn new(registry: Option<&Registry>) -> Self {
        let timing_metrics = TimingBlockMetrics::new(registry, DefaultClock).unwrap_or_else(|e| {
            warn!(
                target: LOG_TARGET,
                "Failed to register Prometheus block timing metrics: {:?}.", e
            );
            TimingBlockMetrics::Noop
        });
        let finality_rate_metrics = FinalityRateMetrics::new(registry).unwrap_or_else(|e| {
            warn!(
                target: LOG_TARGET,
                "Failed to register Prometheus finality rate metrics: {:?}.", e
            );
            FinalityRateMetrics::Noop
        });
        AllBlockMetrics {
            timing_metrics,
            finality_rate_metrics,
        }
    }

    /// Triggers all contained block metrics.
    pub fn report_block(&self, block_id: BlockId, checkpoint: Checkpoint, own: Option<bool>) {
        self.timing_metrics
            .report_block(block_id.hash(), checkpoint);
        self.finality_rate_metrics.report_block(
            block_id.hash(),
            block_id.number(),
            checkpoint,
            own,
        );
    }
}
