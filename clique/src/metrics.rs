use parking_lot::Mutex;
use sc_service::Arc;
use substrate_prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};

struct Inner {
    incoming_connections: Gauge<U64>,
    missing_incoming_connections: Gauge<U64>,
    outgoing_connections: Gauge<U64>,
    missing_outgoing_connections: Gauge<U64>,
}

#[derive(Clone)]
pub struct Metrics {
    inner: Option<Arc<Mutex<Inner>>>,
}

impl Metrics {
    pub fn noop() -> Metrics {
        Metrics { inner: None }
    }

    pub fn new(registry: &Registry) -> Result<Metrics, PrometheusError> {
        let inner = Some(Arc::new(Mutex::new(Inner {
            incoming_connections: register(
                Gauge::new(
                    "clique_network_incoming_connections",
                    "present incoming connections",
                )?,
                registry,
            )?,
            missing_incoming_connections: register(
                Gauge::new(
                    "clique_network_missing_incoming_connections",
                    "(expected-present) incoming connections",
                )?,
                registry,
            )?,
            outgoing_connections: register(
                Gauge::new(
                    "clique_network_outgoing_connections",
                    "present outgoing connections",
                )?,
                registry,
            )?,
            missing_outgoing_connections: register(
                Gauge::new(
                    "clique_network_missing_outgoing_connections",
                    "(expected-present) outgoing connections",
                )?,
                registry,
            )?,
        })));

        Ok(Metrics { inner })
    }

    pub fn set_present_incoming_connections(&self, present: u64) {
        if let Some(ref inner) = self.inner {
            inner.lock().incoming_connections.set(present);
        }
    }

    pub fn set_missing_incoming_connections(&self, missing: u64) {
        if let Some(ref inner) = self.inner {
            inner.lock().missing_incoming_connections.set(missing);
        }
    }

    pub fn set_present_outgoing_connections(&self, present: u64) {
        if let Some(ref inner) = self.inner {
            inner.lock().outgoing_connections.set(present);
        }
    }

    pub fn set_missing_outgoing_connections(&self, missing: u64) {
        if let Some(ref inner) = self.inner {
            inner.lock().missing_outgoing_connections.set(missing);
        }
    }
}
