use std::collections::{hash_map::Entry, HashMap};

use primitives::{BlockHash, BlockNumber};
use substrate_prometheus_endpoint::{register, Counter, PrometheusError, Registry, U64};

use super::Checkpoint;

#[derive(Clone)]
pub enum FinalityRateMetrics {
    Prometheus {
        own_finalized: Counter<U64>,
        own_hopeless: Counter<U64>,
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

    pub fn report_block(
        &mut self,
        block_hash: BlockHash,
        checkpoint: Checkpoint,
        block_number: Option<BlockNumber>,
        own: Option<bool>,
    ) {
        if let Some(number) = block_number {
            match checkpoint {
                Checkpoint::Imported => {
                    if let Some(true) = own {
                        self.report_own_imported(block_hash, number);
                    }
                }
                Checkpoint::Finalized => self.report_finalized(block_hash, number),
                _ => {}
            }
        }
    }

    /// Stores the imported block's hash. Assumes that the imported block is own.
    fn report_own_imported(&mut self, hash: BlockHash, number: BlockNumber) {
        let imported_cache = match self {
            FinalityRateMetrics::Prometheus { imported_cache, .. } => imported_cache,
            FinalityRateMetrics::Noop => return,
        };

        let entry = imported_cache.entry(number).or_default();
        entry.push(hash)
    }

    /// Counts the blocks at the level of `number` different than the passed block
    /// and reports them as hopeless. If `hash` is a hash of own block it will be found
    /// in `imported_cache` and reported as finalized.
    fn report_finalized(&mut self, hash: BlockHash, number: BlockNumber) {
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
                let hashes = entry.get();
                let new_hopeless_count = hashes.iter().filter(|h| **h != hash).count();
                own_hopeless.inc_by(new_hopeless_count as u64);
                own_finalized.inc_by((hashes.len() - new_hopeless_count) as u64);

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
                        .count() as u64,
                );
                entry.remove();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use primitives::{BlockHash, BlockNumber};
    use substrate_prometheus_endpoint::{Counter, Registry, U64};

    use crate::FinalityRateMetrics;

    fn extract_internals(
        metrics: FinalityRateMetrics,
    ) -> (
        Counter<U64>,
        Counter<U64>,
        HashMap<BlockNumber, Vec<BlockHash>>,
    ) {
        match metrics {
            FinalityRateMetrics::Prometheus {
                own_finalized,
                own_hopeless,
                imported_cache,
            } => (own_finalized, own_hopeless, imported_cache),
            FinalityRateMetrics::Noop => panic!("metrics should have been initialized properly"),
        }
    }

    fn verify_state(
        metrics: &FinalityRateMetrics,
        expected_finalized: u64,
        expected_hopeless: u64,
        expected_cache: HashMap<BlockNumber, Vec<BlockHash>>,
    ) {
        let (finalized, hopeless, cache) = extract_internals(metrics.clone());
        assert_eq!(finalized.get(), expected_finalized);
        assert_eq!(hopeless.get(), expected_hopeless);
        assert_eq!(cache, expected_cache);
    }

    #[test]
    fn imported_cache_behaves_properly() {
        let mut metrics = FinalityRateMetrics::new(Some(&Registry::new())).unwrap();

        verify_state(&metrics, 0, 0, HashMap::new());

        let hash0 = BlockHash::random();
        metrics.report_own_imported(hash0, 0);

        verify_state(&metrics, 0, 0, HashMap::from([(0, vec![hash0])]));

        let hash1 = BlockHash::random();
        metrics.report_own_imported(hash1, 1);

        verify_state(
            &metrics,
            0,
            0,
            HashMap::from([(0, vec![hash0]), (1, vec![hash1])]),
        );

        let hash2 = BlockHash::random();
        metrics.report_own_imported(hash2, 1);

        verify_state(
            &metrics,
            0,
            0,
            HashMap::from([(0, vec![hash0]), (1, vec![hash1, hash2])]),
        );

        metrics.report_finalized(hash0, 0);

        verify_state(&metrics, 1, 0, HashMap::from([(1, vec![hash1, hash2])]));

        metrics.report_finalized(BlockHash::random(), 1);

        verify_state(&metrics, 1, 2, HashMap::new());
    }
}
