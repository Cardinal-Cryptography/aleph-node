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
use primitives::BlockNumber;
use sc_service::Arc;
use substrate_prometheus_endpoint::{register, Counter, Gauge, PrometheusError, Registry, U64};

use crate::aleph_primitives::BlockHash;

// How many entries (block hash + timestamp) we keep in memory per one checkpoint type.
// Each entry takes 32B (Hash) + 16B (Instant), so a limit of 5000 gives ~234kB (per checkpoint).
// Notice that some issues like finalization stall may lead to incomplete metrics
// (e.g. when the gap between checkpoints for a block grows over `MAX_BLOCKS_PER_CHECKPOINT`).
const MAX_BLOCKS_PER_CHECKPOINT: usize = 5000;

const LOG_TARGET: &str = "aleph-metrics";

/// TODO(A0-3009): Replace this whole thing.
#[derive(Clone)]
pub enum TimedBlockMetrics {
    Prometheus {
        prev: HashMap<Checkpoint, Checkpoint>,
        gauges: HashMap<Checkpoint, Gauge<U64>>,
        starts: Arc<Mutex<HashMap<Checkpoint, LruCache<BlockHash, Instant>>>>,
    },
    Noop,
}

impl TimedBlockMetrics {
    fn new(registry: Option<&Registry>) -> Result<Self, PrometheusError> {
        use Checkpoint::*;
        let keys = [
            Importing,
            Imported,
            Ordering,
            Ordered,
            Aggregating,
            Finalized,
        ];

        let registry = match registry {
            None => return Ok(Self::Noop),
            Some(registry) => registry,
        };

        let prev: HashMap<_, _> = keys[1..]
            .iter()
            .cloned()
            .zip(keys.iter().cloned())
            .collect();

        let mut gauges = HashMap::new();
        for key in keys.iter() {
            gauges.insert(
                *key,
                register(Gauge::new(format!("aleph_{key:?}"), "no help")?, registry)?,
            );
        }

        Ok(Self::Prometheus {
            prev,
            gauges,
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
        if let TimedBlockMetrics::Prometheus {
            prev,
            gauges,
            starts,
        } = self
        {
            let starts = &mut *starts.lock();
            starts.entry(checkpoint_type).and_modify(|starts| {
                starts.put(hash, checkpoint_time);
            });

            if let Some(prev_checkpoint_type) = prev.get(&checkpoint_type) {
                if let Some(start) = starts
                    .get_mut(prev_checkpoint_type)
                    .expect("All checkpoint types were initialized")
                    .get(&hash)
                {
                    let duration = match checkpoint_time.checked_duration_since(*start) {
                        Some(duration) => duration,
                        None => {
                            warn!(
                                target: LOG_TARGET,
                                "Earlier metrics time {:?} is later that current one \
                            {:?}. Checkpoint type {:?}, block: {:?}",
                                *start,
                                checkpoint_time,
                                checkpoint_type,
                                hash
                            );
                            Duration::new(0, 0)
                        }
                    };
                    gauges
                        .get(&checkpoint_type)
                        .expect("All checkpoint types were initialized")
                        .set(duration.as_millis() as u64);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Checkpoint {
    Importing,
    Imported,
    Ordering,
    Ordered,
    Aggregating,
    Finalized,
}

#[derive(Clone)]
pub enum TopBlockMetrics {
    Prometheus {
        highest_finalized: Counter<U64>,
        best: Gauge<U64>,
    },
    Noop,
}

impl TopBlockMetrics {
    pub fn new(registry: Option<&Registry>) -> Result<Self, PrometheusError> {
        let registry = match registry {
            None => return Ok(Self::Noop),
            Some(registry) => registry,
        };
        Ok(Self::Prometheus {
            highest_finalized: register(
                Counter::new("aleph_highest_finalized_block", "no help")?,
                registry,
            )?,
            best: register(Gauge::new("aleph_best_block", "no help")?, registry)?,
        })
    }

    pub fn update_best(&self, number: BlockNumber) {
        match self {
            TopBlockMetrics::Noop => {}
            TopBlockMetrics::Prometheus { best, .. } => best.set(number as u64),
        }
    }

    pub fn update_highest_finalized(&self, number: BlockNumber) {
        match self {
            TopBlockMetrics::Noop => {}
            TopBlockMetrics::Prometheus {
                highest_finalized, ..
            } => {
                let number = number as u64;
                if number < highest_finalized.get() {
                    warn!(target: LOG_TARGET, "Tried to set highest finalized block to a lower number than before. Resetting `highest_finalized` counter.");
                    highest_finalized.reset();
                }
                let delta = number - highest_finalized.get();
                highest_finalized.inc_by(delta);
            }
        }
    }
}

#[derive(Clone)]
pub struct BlockMetrics {
    pub top_block: TopBlockMetrics,
    pub timed: TimedBlockMetrics,
}

impl BlockMetrics {
    pub fn noop() -> Self {
        Self {
            top_block: TopBlockMetrics::Noop,
            timed: TimedBlockMetrics::Noop,
        }
    }
    pub fn new(registry: Option<&Registry>) -> Result<Self, PrometheusError> {
        Ok(Self {
            top_block: TopBlockMetrics::new(registry)?,
            timed: TimedBlockMetrics::new(registry)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::min;

    use super::*;

    fn register_dummy_metrics() -> BlockMetrics {
        BlockMetrics::new(Some(&Registry::new())).unwrap()
    }

    fn starts_for(m: &BlockMetrics, c: Checkpoint) -> usize {
        match &m.timed {
            TimedBlockMetrics::Prometheus { starts, .. } => starts.lock().get(&c).unwrap().len(),
            _ => 0,
        }
    }

    fn check_reporting_with_memory_excess(metrics: &BlockMetrics, checkpoint: Checkpoint) {
        for i in 1..(MAX_BLOCKS_PER_CHECKPOINT + 10) {
            metrics
                .timed
                .report_block(BlockHash::random(), Instant::now(), checkpoint);
            assert_eq!(
                min(i, MAX_BLOCKS_PER_CHECKPOINT),
                starts_for(metrics, checkpoint)
            )
        }
    }

    #[test]
    fn registration_with_no_register_creates_empty_metrics() {
        let m = BlockMetrics::noop();
        m.timed
            .report_block(BlockHash::random(), Instant::now(), Checkpoint::Ordered);
        assert!(matches!(m.timed, TimedBlockMetrics::Noop));
    }

    #[test]
    fn should_keep_entries_up_to_defined_limit() {
        let m = register_dummy_metrics();
        check_reporting_with_memory_excess(&m, Checkpoint::Ordered);
    }

    #[test]
    fn should_manage_space_for_checkpoints_independently() {
        let m = register_dummy_metrics();
        check_reporting_with_memory_excess(&m, Checkpoint::Ordered);
        check_reporting_with_memory_excess(&m, Checkpoint::Imported);
    }

    #[test]
    fn given_not_monotonic_clock_when_report_block_is_called_repeatedly_code_does_not_panic() {
        let metrics = register_dummy_metrics();
        let earlier_timestamp = Instant::now();
        let later_timestamp = earlier_timestamp + Duration::new(0, 5);
        let hash = BlockHash::random();
        metrics
            .timed
            .report_block(hash, later_timestamp, Checkpoint::Ordering);
        metrics
            .timed
            .report_block(hash, earlier_timestamp, Checkpoint::Ordered);
    }
}
