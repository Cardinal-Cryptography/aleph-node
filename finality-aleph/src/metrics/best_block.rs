use primitives::BlockNumber;
use substrate_prometheus_endpoint::{
    register, Gauge, Histogram, HistogramOpts, PrometheusError, Registry, U64,
};

use crate::{BlockId, SubstrateChainStatus};

#[derive(Clone)]
pub enum BestBlockMetrics {
    Prometheus {
        top_finalized_block: Gauge<U64>,
        best_block: Gauge<U64>,
        reorgs: Histogram,
        best_block_id: BlockId,
        chain_state: SubstrateChainStatus,
    },
    Noop,
}

impl BestBlockMetrics {
    pub fn new(
        registry: Option<Registry>,
        chain_state: SubstrateChainStatus,
    ) -> Result<Self, PrometheusError> {
        let registry = match registry {
            Some(registry) => registry,
            None => return Ok(Self::Noop),
        };

        Ok(Self::Prometheus {
            top_finalized_block: register(
                Gauge::new("aleph_top_finalized_block", "Top finalized block number")?,
                &registry,
            )?,
            best_block: register(
                Gauge::new(
                    "aleph_best_block",
                    "Best (or more precisely, favourite) block number",
                )?,
                &registry,
            )?,
            reorgs: register(
                Histogram::with_opts(
                    HistogramOpts::new("aleph_reorgs", "Number of reorgs by length")
                        .buckets(vec![1., 2., 4., 9.]),
                )?,
                &registry,
            )?,
            best_block_id: (Default::default(), 0u32).into(),
            chain_state,
        })
    }

    pub fn report_best_block_imported(&self, block_id: BlockId, reorg_len: BlockNumber) {
        if let Self::Prometheus {
            best_block, reorgs, ..
        } = self
        {
            best_block.set(block_id.number() as u64);
            if reorg_len > 0 {
                reorgs.observe(reorg_len as f64);
            }
        }
    }

    pub fn report_block_finalized(&self, block_id: BlockId) {
        if let Self::Prometheus {
            top_finalized_block,
            ..
        } = self
        {
            top_finalized_block.set(block_id.number() as u64);
        }
    }
}
