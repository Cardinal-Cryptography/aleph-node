use futures::{future, StreamExt};
use log::warn;
use sc_client_api::{FinalityNotifications, ImportNotifications};
use sp_api::{BlockT, HeaderT};
use sp_blockchain::{lowest_common_ancestor, HeaderMetadata};
use substrate_prometheus_endpoint::{
    register, Gauge, Histogram, HistogramOpts, PrometheusError, Registry, U64,
};
use tokio::select;

use crate::{metrics::LOG_TARGET, BlockNumber};

pub enum ChainStatusMetrics {
    Prometheus {
        top_finalized_block: Gauge<U64>,
        best_block: Gauge<U64>,
        reorgs: Histogram,
    },
    Noop,
}

impl ChainStatusMetrics {
    pub fn new(registry: Option<Registry>) -> Result<Self, PrometheusError> {
        let registry = match registry {
            Some(registry) => registry,
            None => return Ok(ChainStatusMetrics::Noop),
        };

        Ok(ChainStatusMetrics::Prometheus {
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

    pub fn try_new_of_default_with_warning_logged(registry: Option<Registry>) -> Self {
        match Self::new(registry) {
            Ok(metrics) => metrics,
            Err(e) => {
                warn!(target: LOG_TARGET, "Failed to create metrics: {e}.");
                Self::noop()
            }
        }
    }

    pub fn noop() -> Self {
        ChainStatusMetrics::Noop
    }

    pub fn update_best_block(&self, number: BlockNumber) {
        if let ChainStatusMetrics::Prometheus { best_block, .. } = self {
            best_block.set(number as u64)
        }
    }

    pub fn update_top_finalized_block(&self, number: BlockNumber) {
        if let ChainStatusMetrics::Prometheus {
            top_finalized_block,
            ..
        } = self
        {
            top_finalized_block.set(number as u64);
        }
    }

    pub fn report_reorg(&self, length: BlockNumber) {
        if let ChainStatusMetrics::Prometheus { reorgs, .. } = self {
            reorgs.observe(length as f64);
        }
    }
}

pub async fn start_chain_state_metrics_job_in_current_thread<
    HE: HeaderT<Number = u32, Hash = B::Hash>,
    B: BlockT<Header = HE>,
    BE: HeaderMetadata<B>,
>(
    backend: &BE,
    import_notifications: ImportNotifications<B>,
    finality_notifications: FinalityNotifications<B>,
    metrics: ChainStatusMetrics,
) {
    let mut best_block_notifications = import_notifications
        .fuse()
        .filter(|notification| future::ready(notification.is_new_best));
    let mut finality_notifications = finality_notifications.fuse();

    let mut previous_best: Option<B::Hash> = None;
    loop {
        select! {
            maybe_block = best_block_notifications.next() => {
                match maybe_block {
                    Some(block) => {
                        // If best block was changed in the meantime by finalization, then we will
                        // update best block to correct value and detect reorg (of the same length)
                        // only now.
                        let number: <<B as BlockT>::Header as HeaderT>::Number = *block.header.number();
                        metrics.update_best_block(number);
                        if let Some(reorg_len) = detect_reorgs(backend, previous_best, block.header.clone()) {
                            metrics.report_reorg(reorg_len);
                        }
                        previous_best = Some(block.hash);
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
                        // we also don't update there. Best block will only be updated after
                        // importing something on the newly finalized branch.
                        // metrics.update_top_finalized();
                        let number: <<B as BlockT>::Header as HeaderT>::Number = *block.header.number();
                        metrics.update_top_finalized_block(number);
                    }
                    None => {
                        warn!(target: LOG_TARGET, "Finality notification stream ended unexpectedly");
                    }
                }
            },
        }
    }
}

fn detect_reorgs<
    HE: HeaderT<Number = u32, Hash = B::Hash>,
    B: BlockT<Header = HE>,
    BE: HeaderMetadata<B>,
>(
    backend: &BE,
    previous_best_hash: Option<B::Hash>,
    best: HE,
) -> Option<HE::Number> {
    let previous_best_hash = previous_best_hash?;
    if *best.parent_hash() == previous_best_hash || best.hash() == previous_best_hash {
        // Quit early when no change or a child
        return None;
    }
    let lca = lowest_common_ancestor(backend, best.hash(), previous_best_hash).ok()?;
    match best.number() - lca.number {
        0 => None,
        len => Some(len),
    }
}
