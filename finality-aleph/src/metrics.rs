use log::trace;
use lru::LruCache;
use parking_lot::Mutex;
use prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};
use sc_service::Arc;
use sp_runtime::traits::Header;
use std::{collections::HashMap, time::Instant};

// How many entries (block hash + timestamp) we keep in memory per one checkpoint type.
const MAX_BLOCKS_PER_CHECKPOINT: usize = 20;

struct Inner<H: Header> {
    prev: HashMap<Checkpoint, Checkpoint>,
    gauges: HashMap<Checkpoint, Gauge<U64>>,
    starts: HashMap<Checkpoint, LruCache<H::Hash, Instant>>,
}

impl<H: Header> Inner<H> {
    fn report_block(
        &mut self,
        hash: H::Hash,
        checkpoint_time: Instant,
        checkpoint_type: Checkpoint,
    ) {
        trace!(target: "afa", "Reporting block stage: {:?} (hash: {:?}, at: {:?}", checkpoint_type, hash, checkpoint_time);

        self.starts.entry(checkpoint_type).and_modify(|starts| {
            starts.put(hash, checkpoint_time);
        });

        if let Some(prev_checkpoint_type) = self.prev.get(&checkpoint_type) {
            if let Some(start) = self
                .starts
                .get_mut(prev_checkpoint_type)
                .expect("All checkpoint types were initialized")
                .get(&hash)
            {
                self.gauges
                    .get(&checkpoint_type)
                    .expect("All checkpoint types were initialized")
                    .set(checkpoint_time.duration_since(*start).as_millis() as u64);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Checkpoint {
    Importing,
    Imported,
    Ordering,
    Ordered,
    Aggregating,
    Finalized,
}

#[derive(Clone)]
pub struct Metrics<H: Header> {
    inner: Arc<Mutex<Inner<H>>>,
}

impl<H: Header> Metrics<H> {
    pub fn register(registry: &Registry) -> Result<Self, PrometheusError> {
        use Checkpoint::*;
        let keys = [
            Importing,
            Imported,
            Ordering,
            Ordered,
            Aggregating,
            Finalized,
        ];
        let prev: HashMap<_, _> = keys[1..]
            .iter()
            .cloned()
            .zip(keys.iter().cloned())
            .collect();

        let mut gauges = HashMap::new();
        for key in keys.iter() {
            gauges.insert(
                *key,
                register(Gauge::new(format!("aleph_{:?}", key), "no help")?, registry)?,
            );
        }

        let inner = Arc::new(Mutex::new(Inner {
            prev,
            gauges,
            starts: keys
                .iter()
                .map(|k| (*k, LruCache::new(MAX_BLOCKS_PER_CHECKPOINT)))
                .collect(),
        }));

        Ok(Self { inner })
    }

    pub(crate) fn report_block(
        &self,
        hash: H::Hash,
        checkpoint_time: Instant,
        checkpoint_type: Checkpoint,
    ) {
        self.inner
            .lock()
            .report_block(hash, checkpoint_time, checkpoint_type);
    }
}
