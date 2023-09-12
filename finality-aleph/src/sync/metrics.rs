use std::collections::HashMap;

use log::warn;
use substrate_prometheus_endpoint::{register, Counter, Gauge, PrometheusError, Registry, U64};
const LOG_TARGET: &str = "aleph-metrics";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Event {
    Broadcast,
    SendRequest,
    SendTo,
    HandleState,
    HandleRequestResponse,
    HandleRequest,
    HandleTask,
    HandleBlockImported,
    HandleBlockFinalized,
    HandleStateResponse,
    HandleJustificationFromUser,
    HandleInternalRequest,
}

use Event::*;

use crate::BlockNumber;

impl Event {
    fn name(&self) -> &str {
        match self {
            Broadcast => "broadcast",
            SendRequest => "send_request",
            SendTo => "send_to",
            HandleState => "handle_state",
            HandleRequestResponse => "handle_request_response",
            HandleRequest => "handle_request",
            HandleTask => "handle_task",
            HandleBlockImported => "handle_block_imported",
            HandleBlockFinalized => "handle_block_finalized",
            HandleStateResponse => "handle_state_response",
            HandleJustificationFromUser => "handle_justification_from_user",
            HandleInternalRequest => "handle_internal_request",
        }
    }
}

const ALL_EVENTS: [Event; 12] = [
    Broadcast,
    SendRequest,
    SendTo,
    HandleState,
    HandleRequestResponse,
    HandleRequest,
    HandleTask,
    HandleBlockImported,
    HandleBlockFinalized,
    HandleStateResponse,
    HandleJustificationFromUser,
    HandleInternalRequest,
];

const ERRORING_EVENTS: [Event; 9] = [
    Broadcast,
    SendRequest,
    SendTo,
    HandleState,
    HandleRequest,
    HandleTask,
    HandleBlockImported,
    HandleJustificationFromUser,
    HandleInternalRequest,
];

pub enum Metrics {
    Prometheus {
        calls: HashMap<Event, Counter<U64>>,
        errors: HashMap<Event, Counter<U64>>,
    },
    Noop,
}

impl Metrics {
    pub fn new(registry: Option<Registry>) -> Result<Self, PrometheusError> {
        let registry = match registry {
            Some(registry) => registry,
            None => return Ok(Metrics::Noop),
        };
        let mut calls = HashMap::new();
        let mut errors = HashMap::new();
        for event in ALL_EVENTS {
            calls.insert(
                event,
                register(
                    Counter::new(
                        format!("aleph_sync_{}", event.name()),
                        format!("number of times {} has been called", event.name()),
                    )?,
                    &registry,
                )?,
            );
        }
        for event in ERRORING_EVENTS {
            errors.insert(
                event,
                register(
                    Counter::new(
                        format!("aleph_sync_{}_error", event.name()),
                        format!("number of times {} has returned an error", event.name()),
                    )?,
                    &registry,
                )?,
            );
        }
        Ok(Metrics::Prometheus { calls, errors })
    }

    pub fn report_event(&self, event: Event) {
        if let Metrics::Prometheus { calls, .. } = self {
            if let Some(counter) = calls.get(&event) {
                counter.inc();
            }
        }
    }

    pub fn report_event_error(&self, event: Event) {
        if let Metrics::Prometheus { errors, .. } = self {
            if let Some(counter) = errors.get(&event) {
                counter.inc();
            }
        }
    }
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
                Counter::new("aleph_top_finalized_block", "no help")?,
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

    pub fn get_best(&self) -> BlockNumber {
        match self {
            TopBlockMetrics::Noop => 0,
            TopBlockMetrics::Prometheus { best, .. } => best.get().try_into().unwrap(),
        }
    }

    pub fn update_top_finalized(&self, number: BlockNumber) {
        match self {
            TopBlockMetrics::Noop => {}
            TopBlockMetrics::Prometheus {
                highest_finalized, ..
            } => {
                let number = number as u64;
                if number < highest_finalized.get() {
                    warn!(
                        target: LOG_TARGET,
                        "Tried to set highest finalized block to a lower number than before."
                    );
                } else {
                    let delta = number - highest_finalized.get();
                    highest_finalized.inc_by(delta);
                }
            }
        }
    }
}
