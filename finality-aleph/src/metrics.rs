use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use log::{trace, warn};
use lru::LruCache;
use parking_lot::Mutex;
use sc_service::Arc;
use substrate_prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};

use crate::aleph_primitives::BlockHash;

// How many entries (block hash + timestamp) we keep in memory per one checkpoint type.
// Each entry takes 32B (Hash) + 16B (Instant), so a limit of 5000 gives ~234kB (per checkpoint).
// Notice that some issues like finalization stall may lead to incomplete metrics
// (e.g. when the gap between checkpoints for a block grows over `MAX_BLOCKS_PER_CHECKPOINT`).
const MAX_BLOCKS_PER_CHECKPOINT: usize = 5000;

const LOG_TARGET: &str = "aleph-metrics";

/// TODO(A0-3009): Improve BlockMetrics and rename to TimedBlockMetrics or such
#[derive(Clone)]
pub enum BlockMetrics {
    Prometheus {
        time_since_prev_checkpoint: HashMap<Checkpoint, Gauge<U64>>,
        imported_to_finalized: Gauge<U64>,
        starts: Arc<Mutex<HashMap<Checkpoint, LruCache<BlockHash, Instant>>>>,
    },
    Noop,
}

impl BlockMetrics {
    pub fn new(registry: Option<&Registry>) -> Result<Self, PrometheusError> {
        use Checkpoint::*;
        let keys = [Importing, Imported, Ordering, Ordered, Finalized];

        let registry = match registry {
            None => return Ok(Self::Noop),
            Some(registry) => registry,
        };

        let mut time_since_prev_checkpoint = HashMap::new();
        for key in keys.iter() {
            time_since_prev_checkpoint.insert(
                *key,
                register(Gauge::new(format!("aleph_{key:?}"), "no help")?, registry)?,
            );
        }

        Ok(Self::Prometheus {
            time_since_prev_checkpoint,
            imported_to_finalized: register(
                Gauge::new("aleph_Imported_to_Finalized", "no help")?,
                registry,
            )?,
            starts: Arc::new(Mutex::new(
                keys.iter()
                    .map(|k| {
                        (
                            *k,
                            LruCache::new(NonZeroUsize::new(MAX_BLOCKS_PER_CHECKPOINT).unwrap()),
                        )
                    })
                    .collect(),
            )),
        })
    }

    pub fn noop() -> Self {
        Self::Noop
    }

    pub fn report_block(
        &self,
        hash: BlockHash,
        checkpoint_time: Instant,
        checkpoint_type: Checkpoint,
    ) {
        trace!(
            target: LOG_TARGET,
            "Reporting block stage: {:?} (hash: {:?}, at: {:?}",
            checkpoint_type,
            hash,
            checkpoint_time
        );
        let (time_since_prev_checkpoint, imported_to_finalized, starts) = match self {
            BlockMetrics::Noop => return,
            BlockMetrics::Prometheus {
                time_since_prev_checkpoint,
                imported_to_finalized,
                starts,
            } => (time_since_prev_checkpoint, imported_to_finalized, starts),
        };

        let starts = &mut *starts.lock();
        starts.entry(checkpoint_type).and_modify(|starts| {
            starts.put(hash, checkpoint_time);
        });

        if let Some(prev_checkpoint_type) = checkpoint_type.prev() {
            if let Some(start) = starts
                .get_mut(&prev_checkpoint_type)
                .expect("All checkpoint types were initialized")
                .get(&hash)
            {
                let duration = checkpoint_time
                    .checked_duration_since(*start)
                    .unwrap_or_else(|| {
                        Self::warn_about_monotonicity_violation(
                            *start,
                            checkpoint_time,
                            checkpoint_type,
                            hash,
                        );
                        Duration::new(0, 0)
                    });
                time_since_prev_checkpoint
                    .get(&checkpoint_type)
                    .expect("All checkpoint types were initialized")
                    .set(duration.as_millis() as u64);
            }
        }
        if checkpoint_type == Checkpoint::Finalized {
            if let Some(start) = starts
                .get_mut(&Checkpoint::Imported)
                .expect("All checkpoint types were initialized")
                .get(&hash)
            {
                let duration = checkpoint_time
                    .checked_duration_since(*start)
                    .unwrap_or_else(|| {
                        Self::warn_about_monotonicity_violation(
                            *start,
                            checkpoint_time,
                            checkpoint_type,
                            hash,
                        );
                        Duration::new(0, 0)
                    });
                imported_to_finalized.set(duration.as_millis() as u64);
            }
        }
    }

    fn warn_about_monotonicity_violation(
        start: Instant,
        checkpoint_time: Instant,
        checkpoint_type: Checkpoint,
        hash: BlockHash,
    ) {
        warn!(
            target: LOG_TARGET,
            "Earlier metrics time {:?} is later that current one \
        {:?}. Checkpoint type {:?}, block: {:?}",
            start,
            checkpoint_time,
            checkpoint_type,
            hash
        );
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Checkpoint {
    Importing,
    Imported,
    Ordering,
    Ordered,
    Finalized,
}

impl Checkpoint {
    fn prev(&self) -> Option<Checkpoint> {
        use Checkpoint::*;
        match self {
            Importing => None,
            Imported => Some(Importing),
            Ordering => Some(Imported),
            Ordered => Some(Ordering),
            Finalized => Some(Ordered),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::min;

    use super::*;

    fn register_prometheus_metrics_with_dummy_registry() -> BlockMetrics {
        BlockMetrics::new(Some(&Registry::new())).unwrap()
    }

    fn starts_for(m: &BlockMetrics, c: Checkpoint) -> usize {
        match &m {
            BlockMetrics::Prometheus { starts, .. } => starts.lock().get(&c).unwrap().len(),
            _ => 0,
        }
    }

    fn check_reporting_with_memory_excess(metrics: &BlockMetrics, checkpoint: Checkpoint) {
        for i in 1..(MAX_BLOCKS_PER_CHECKPOINT + 10) {
            metrics.report_block(BlockHash::random(), Instant::now(), checkpoint);
            assert_eq!(
                min(i, MAX_BLOCKS_PER_CHECKPOINT),
                starts_for(metrics, checkpoint)
            )
        }
    }

    #[test]
    fn noop_metrics() {
        let m = BlockMetrics::noop();
        m.report_block(BlockHash::random(), Instant::now(), Checkpoint::Ordered);
        assert!(matches!(m, BlockMetrics::Noop));
    }

    #[test]
    fn should_keep_entries_up_to_defined_limit() {
        let m = register_prometheus_metrics_with_dummy_registry();
        check_reporting_with_memory_excess(&m, Checkpoint::Ordered);
    }

    #[test]
    fn should_manage_space_for_checkpoints_independently() {
        let m = register_prometheus_metrics_with_dummy_registry();
        check_reporting_with_memory_excess(&m, Checkpoint::Ordered);
        check_reporting_with_memory_excess(&m, Checkpoint::Imported);
    }

    #[test]
    fn given_not_monotonic_clock_when_report_block_is_called_repeatedly_code_does_not_panic() {
        let metrics = register_prometheus_metrics_with_dummy_registry();
        let earlier_timestamp = Instant::now();
        let later_timestamp = earlier_timestamp + Duration::new(0, 5);
        let hash = BlockHash::random();
        metrics.report_block(hash, later_timestamp, Checkpoint::Ordering);
        metrics.report_block(hash, earlier_timestamp, Checkpoint::Ordered);
    }
}
