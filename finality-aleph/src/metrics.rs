use prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};
use sp_runtime::traits::Header;
use std::{collections::HashMap, time::Instant};

#[derive(Clone)]
pub struct Metrics<H: Header> {
    keys: [&'static str; 5],
    prev: HashMap<&'static str, &'static str>,
    pub gauges: HashMap<&'static str, Gauge<U64>>,
    starts: HashMap<&'static str, HashMap<H::Hash, Instant>>,
}

impl<H: Header> Metrics<H> {
    pub fn register(registry: &Registry) -> Result<Self, PrometheusError> {
        let keys = [
            "importing",
            "imported",
            "get_data",
            "finalize",
            "aggregation-start",
        ];
        let prev: HashMap<&str, &str> = [
            ("imported", "importing"),
            ("get_data", "imported"),
            ("aggregation-start", "get_data"),
            ("finalize", "aggregation-start"),
        ]
        .iter()
        .cloned()
        .collect();

        let mut gauges = HashMap::new();
        for key in keys.iter() {
            gauges.insert(
                *key,
                register(Gauge::new(format!("aleph_{}", *key), "no help")?, registry)?,
            );
        }

        Ok(Self {
            keys,
            prev,
            gauges,
            starts: keys.iter().map(|k| (*k, HashMap::new())).collect(),
        })
    }

    pub fn report_block(
        &mut self,
        hash: H::Hash,
        checkpoint: Instant,
        checkpoint_name: &'static str,
    ) {
        log::debug!(target: "afa", "Reporting block stage: {} (hash: {:?}, at: {:?}", checkpoint_name, hash, checkpoint);

        self.starts.entry(checkpoint_name).and_modify(|starts| {
            starts.entry(hash).or_insert(checkpoint);
        });

        if let Some(prev_checkpoint_name) = self.prev.get(checkpoint_name) {
            if let Some(start) = self
                .starts
                .get(prev_checkpoint_name)
                .expect("prev was stored")
                .get(&hash)
            {
                self.gauges
                    .get(checkpoint_name)
                    .expect("checkpoint gauge was stored")
                    .set(checkpoint.duration_since(*start).as_millis() as u64);
            }
        }
    }
}
