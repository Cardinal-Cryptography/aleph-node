use substrate_prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};

#[derive(Clone)]
pub enum Metrics {
    Noop,
    Metrics {
        incoming_connections: Gauge<U64>,
        missing_incoming_connections: Gauge<U64>,
        outgoing_connections: Gauge<U64>,
        missing_outgoing_connections: Gauge<U64>,
    },
}

impl Metrics {
    pub fn new(registry: Option<Registry>) -> Result<Self, PrometheusError> {
        match registry {
            Some(registry) => Ok(Metrics::Metrics {
                incoming_connections: register(
                    Gauge::new(
                        "clique_network_incoming_connections",
                        "present incoming connections",
                    )?,
                    &registry,
                )?,
                missing_incoming_connections: register(
                    Gauge::new(
                        "clique_network_missing_incoming_connections",
                        "difference between expected and present incoming connections",
                    )?,
                    &registry,
                )?,
                outgoing_connections: register(
                    Gauge::new(
                        "clique_network_outgoing_connections",
                        "present outgoing connections",
                    )?,
                    &registry,
                )?,
                missing_outgoing_connections: register(
                    Gauge::new(
                        "clique_network_missing_outgoing_connections",
                        "difference between expected and present outgoing connections",
                    )?,
                    &registry,
                )?,
            }),
            None => Ok(Metrics::Noop),
        }
    }

    pub fn noop() -> Self {
        Metrics::Noop
    }

    pub fn set_incoming_connections(&self, present: u64) {
        if let Metrics::Metrics {
            incoming_connections,
            ..
        } = self
        {
            incoming_connections.set(present);
        }
    }

    pub fn set_missing_incoming_connections(&self, missing: u64) {
        if let Metrics::Metrics {
            missing_incoming_connections,
            ..
        } = self
        {
            missing_incoming_connections.set(missing);
        }
    }

    pub fn set_outgoing_connections(&self, present: u64) {
        if let Metrics::Metrics {
            outgoing_connections,
            ..
        } = self
        {
            outgoing_connections.set(present);
        }
    }

    pub fn set_missing_outgoing_connections(&self, missing: u64) {
        if let Metrics::Metrics {
            missing_outgoing_connections,
            ..
        } = self
        {
            missing_outgoing_connections.set(missing);
        }
    }
}
