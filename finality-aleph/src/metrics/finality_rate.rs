use std::collections::{hash_map::Entry, HashMap};

use primitives::{BlockHash, BlockNumber};
use substrate_prometheus_endpoint::{prometheus::Counter, register, PrometheusError, Registry};

#[derive(Clone)]
pub enum FinalityRateMetrics {
    Prometheus {
        own_finalized: Counter,
        own_hopeless: Counter,
        imported_cache: HashMap<BlockNumber, Vec<BlockHash>>,
    },
    Noop,
}

impl FinalityRateMetrics {
    pub fn new(registry: Option<&Registry>) -> Result<Self, PrometheusError> {
        let registry = match registry {
            None => return Ok(FinalityRateMetrics::Noop),
            Some(registry) => registry,
        };

        Ok(FinalityRateMetrics::Prometheus {
            own_finalized: register(
                Counter::new("aleph_own_finalized_blocks", "no help")?,
                registry,
            )?,
            own_hopeless: register(
                Counter::new("aleph_own_hopeless_blocks", "no help")?,
                registry,
            )?,
            imported_cache: HashMap::new(),
        })
    }

    pub fn report_own_imported(&mut self, hash: BlockHash, number: BlockNumber) {
        let imported_cache = match self {
            FinalityRateMetrics::Prometheus { imported_cache, .. } => imported_cache,
            FinalityRateMetrics::Noop => return,
        };

        match imported_cache.entry(number) {
            Entry::Occupied(mut entry) => entry.get_mut().push(hash),
            Entry::Vacant(entry) => {
                entry.insert(vec![hash]);
            }
        }
    }

    pub fn report_finalized(&mut self, hash: BlockHash, number: BlockNumber) {
        let (own_finalized, own_hopeless, imported_cache) = match self {
            FinalityRateMetrics::Prometheus {
                own_finalized,
                own_hopeless,
                imported_cache,
            } => (own_finalized, own_hopeless, imported_cache),
            FinalityRateMetrics::Noop => return,
        };

        match imported_cache.entry(number) {
            Entry::Occupied(entry) => {
                own_hopeless.inc_by(
                    entry
                        .get()
                        .iter()
                        .filter(|h| {
                            if **h == hash {
                                own_finalized.inc();
                            }
                            **h != hash
                        })
                        .count() as f64,
                );
                entry.remove();
            }
            Entry::Vacant(_) => (),
        }
    }
}
