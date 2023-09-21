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
use substrate_prometheus_endpoint::{
    register, Histogram, HistogramOpts, PrometheusError, Registry,
};

use crate::{aleph_primitives::BlockHash, Display};

// How many entries (block hash + timestamp) we keep in memory per one checkpoint type.
// Each entry takes 32B (Hash) + 16B (Instant), so a limit of 5000 gives ~234kB (per checkpoint).
// Notice that some issues like finalization stall may lead to incomplete metrics
// (e.g. when the gap between checkpoints for a block grows over `MAX_BLOCKS_PER_CHECKPOINT`).
const MAX_BLOCKS_PER_CHECKPOINT: usize = 5000;

const LOG_TARGET: &str = "aleph-metrics";
const HISTOGRAM_BUCKETS: [f64; 11] = [
    1., 5., 25., 100., 150., 250., 500., 1000., 2000., 5000., 10000.,
];

#[derive(Clone)]
pub enum BlockMetrics {
    Prometheus {
        time_since_prev_checkpoint: HashMap<Checkpoint, Histogram>,
        imported_to_finalized: Histogram,
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
        for key in keys[1..].iter() {
            time_since_prev_checkpoint.insert(
                *key,
                register(
                    Histogram::with_opts(
                        HistogramOpts::new(
                            format!("aleph_{:?}", key.to_string().make_ascii_lowercase()),
                            "no help",
                        )
                        .buckets(HISTOGRAM_BUCKETS.to_vec()),
                    )?,
                    registry,
                )?,
            );
        }

        Ok(Self::Prometheus {
            time_since_prev_checkpoint,
            imported_to_finalized: register(
                Histogram::with_opts(
                    HistogramOpts::new("aleph_imported_to_finalized", "no help")
                        .buckets(HISTOGRAM_BUCKETS.to_vec()),
                )?,
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

    /// Updates metrics for `checkpoint` so that all blocks referenced previously with `header_hash`
    /// are now referenced with `post_hash`. This is useful when a block is first reported
    /// with header hash not including post-digests, and later it is reported with header hash
    /// including post digests.
    pub fn convert_header_hash_to_post_hash(
        &self,
        header_hash: BlockHash,
        post_hash: BlockHash,
        checkpoint: Checkpoint,
    ) {
        let starts = match self {
            BlockMetrics::Noop => return,
            BlockMetrics::Prometheus { starts, .. } => starts,
        };
        let starts = &mut starts.lock();
        let checkpoint_map = starts
            .get_mut(&checkpoint)
            .expect("All checkpoint types were initialized");

        if let Some((_, time)) = checkpoint_map.pop_entry(&header_hash) {
            checkpoint_map.push(post_hash, time);
        }
    }

    pub fn report_block_if_not_present(
        &self,
        hash: BlockHash,
        checkpoint_time: Instant,
        checkpoint_type: Checkpoint,
    ) {
        let starts = match self {
            BlockMetrics::Noop => return,
            BlockMetrics::Prometheus { starts, .. } => starts,
        };
        if !starts
            .lock()
            .get_mut(&checkpoint_type)
            .expect("All checkpoint types were initialized")
            .contains(&hash)
        {
            self.report_block(hash, checkpoint_time, checkpoint_type);
        }
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
                    .observe(duration.as_secs_f64() / 1000.);
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
                imported_to_finalized.observe(duration.as_secs_f64() / 1000.);
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

#[derive(Clone, Copy, Debug, Display, Hash, PartialEq, Eq)]
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

    use Checkpoint::*;

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
        m.report_block(BlockHash::random(), Instant::now(), Ordered);
        assert!(matches!(m, BlockMetrics::Noop));
    }

    #[test]
    fn should_keep_entries_up_to_defined_limit() {
        let m = register_prometheus_metrics_with_dummy_registry();
        check_reporting_with_memory_excess(&m, Ordered);
    }

    #[test]
    fn should_manage_space_for_checkpoints_independently() {
        let m = register_prometheus_metrics_with_dummy_registry();
        check_reporting_with_memory_excess(&m, Ordered);
        check_reporting_with_memory_excess(&m, Imported);
    }

    #[test]
    fn given_not_monotonic_clock_when_report_block_is_called_repeatedly_code_does_not_panic() {
        let metrics = register_prometheus_metrics_with_dummy_registry();
        let earlier_timestamp = Instant::now();
        let later_timestamp = earlier_timestamp + Duration::new(0, 5);
        let hash = BlockHash::random();
        metrics.report_block(hash, later_timestamp, Ordering);
        metrics.report_block(hash, earlier_timestamp, Ordered);
    }

    #[test]
    fn tests_hash_conversion() {
        let metrics = register_prometheus_metrics_with_dummy_registry();
        let timestamp1 = Instant::now();
        let timestamp2 = timestamp1 + Duration::new(0, 5);

        let hash = [
            BlockHash::random(),
            BlockHash::random(),
            BlockHash::random(),
        ];

        metrics.report_block(hash[0], timestamp1, Ordering);
        metrics.report_block(hash[1], timestamp2, Ordering);

        metrics.convert_header_hash_to_post_hash(hash[0], hash[2], Ordering);

        let entries = match &metrics {
            BlockMetrics::Prometheus { starts, .. } => starts
                .lock()
                .get(&Ordering)
                .unwrap()
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>(),
            _ => vec![],
        };
        assert_eq!(entries, &[(hash[2], timestamp1), (hash[1], timestamp2)]);
    }

    #[test]
    fn test_report_block_if_not_present() {
        let metrics = register_prometheus_metrics_with_dummy_registry();
        let earlier_timestamp = Instant::now();
        let later_timestamp = earlier_timestamp + Duration::new(0, 5);
        let hash = BlockHash::random();

        metrics.report_block(hash, earlier_timestamp, Ordering);
        metrics.report_block_if_not_present(hash, later_timestamp, Ordered);

        let timestamp = match &metrics {
            BlockMetrics::Prometheus { starts, .. } => starts
                .lock()
                .get_mut(&Ordering)
                .unwrap()
                .get(&hash)
                .cloned(),
            _ => None,
        };
        assert_eq!(timestamp, Some(earlier_timestamp));
    }
}
