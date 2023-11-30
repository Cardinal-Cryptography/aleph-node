use log::warn;
use primitives::{BlockHash, BlockNumber};
use substrate_prometheus_endpoint::Registry;

use super::{finality_rate::FinalityRateMetrics, timing::DefaultClock, Checkpoint};
use crate::{metrics::LOG_TARGET, TimingBlockMetrics};

/// Wrapper around various block-related metrics.
#[derive(Clone)]
pub struct AllBlockMetrics {
    timing_metrics: TimingBlockMetrics<DefaultClock>,
    finality_rate_metrics: FinalityRateMetrics,
}

impl AllBlockMetrics {
    pub fn new(registry: Option<&Registry>) -> Self {
        let timing_metrics = match TimingBlockMetrics::new(registry, DefaultClock) {
            Ok(timing_metrics) => timing_metrics,
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Failed to register Prometheus block timing metrics: {:?}.", e
                );
                TimingBlockMetrics::Noop
            }
        };
        let finality_rate_metrics = match FinalityRateMetrics::new(registry) {
            Ok(finality_rate_metrics) => finality_rate_metrics,
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Failed to register Prometheus finality rate metrics: {:?}.", e
                );
                FinalityRateMetrics::Noop
            }
        };
        AllBlockMetrics {
            timing_metrics,
            finality_rate_metrics,
        }
    }

    /// Triggers all contained block metrics.
    pub fn report_block(
        &self,
        block_hash: BlockHash,
        checkpoint: Checkpoint,
        block_number: Option<BlockNumber>,
        own: Option<bool>,
    ) {
        self.timing_metrics.report_block(block_hash, checkpoint);
        self.finality_rate_metrics
            .report_block(block_hash, checkpoint, block_number, own);
    }
}
