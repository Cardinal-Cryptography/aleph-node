use crate::{nodes::NodeIndex, AuthorityId};
use log::debug;
use parking_lot::RwLock;
use prometheus_endpoint::{register, CounterVec, Opts, PrometheusError, Registry, U64};
use sc_network::{ObservedRole, PeerId};
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};
use sp_core::traits::BareCryptoStorePtr;
use sp_runtime::traits::{Block, CheckedConversion, NumberFor, Zero};
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use rand::seq::SliceRandom;

const REBROADCAST_AFTER: Duration = Duration::from_secs(60 * 5);

enum MessageAction {
    Accept,
    RejectPast,
    RejectFuture,
    RejectOutOfScope,
}

// TODO
struct GlobalViewState<N> {}

// TODO
struct LocalViewState {}

struct PeerInfo<N> {
    view: GlobalViewState<N>,
}

struct Peers<N>(HashMap<PeerId, PeerInfo<N>>);

impl<N> Peers<N> {
    fn insert_peer(&mut self, who: PeerId, role: ObservedRole) {
        self.0.insert(who, PeerInfo::new(role));
    }

    fn remove_peer(&mut self, who: &PeerId) {
        self.0.remove(who.as_ref());
    }

    fn get_peer(&self, who: &PeerId) -> Option<&PeerInfo<N>> {
        self.0.get(who.as_ref())
    }

    fn authorities(&self) -> usize {
        self.0
            .iter()
            .filter(|_, info| matches!(info.roles, ObservedRole::Authority))
            .count()
    }

    fn non_authorities(&self) -> usize {
        self.0
            .iter()
            .filter(|(_, info)| matches!(info.roles, ObservedRole::Full | ObservedRole::Light))
            .count()
    }

    fn shuffle(&mut self) {
        let mut peers = self.0.clone();
        peers.partial_shuffle(&mut rand::thread_rng(), self.0.len());
        peers.truncate(self.0.len());
        self.0.clear();
        self.0.extend(peers.into_iter());
    }
}

// TODO
enum PeerAction {
    Keep,
    ProcessAndDiscard,
    Discard,
}

// TODO
enum PendingSync {
    None,
    Requesting,
    Processing,
}

enum SyncConfig {
    Enabled { only_from_authorities: bool },
    Disabled,
}

// TODO
struct Unit<H> {}

struct FetchRequest {
    hashes: Vec<NodeIndex>,
    unit_id: NodeIndex,
}

struct FetchResponse<H> {
    hashes: Vec<Unit<H>>,
    unit_id: NodeIndex,
}

// TODO
struct SyncRequest {}

// TODO
struct SyncResponse {}

// TODO
struct Alert {}

// TODO
struct PeerReport {}

type PrometheusResult<T> = Result<T, PrometheusError>;

// TODO
struct Metrics {
    messages_validated: CounterVec<U64>,
}

impl Metrics {
    pub(crate) fn register(registry: &prometheus_endpoint::Registry) -> PrometheusResult<Self> {
        Ok(Self {
            messages_validated: register(
                CounterVec::new(
                    Opts::new(
                        "finality_aleph_communication_gossip_validator_messages",
                        "Number of messages validated by the finality aleph gossip validator.",
                    ),
                    &["message", "action"],
                )?,
                registry,
            )?,
        })
    }
}

pub(super) enum GossipMessage<H> {
    FetchRequest(FetchRequest),
    FetchResponse(FetchResponse<H>),
    SyncRequest(SyncRequest),
    SyncResponse(SyncResponse),
    Alert(Alert),
}

struct GossipValidatorConfig {
    pub gossip_duration: Duration,
    pub justification_period: u32,
    pub observer_enabled: bool,
    pub is_authority: bool,
    pub name: Option<String>,
    pub keystore: Option<BareCryptoStorePtr>,
}

pub(super) struct GossipValidator<B: Block> {
    // local_view: RwLock<Option<LocalViewState>>,
    peers: RwLock<Peers<NumberFor<B>>>,
    // live_topics // TODO
    authorities: RwLock<Vec<AuthorityId>>,
    config: RwLock<GossipValidatorConfig>,
    next_rebroadcast: RwLock<Instant>,
    pending_sync: RwLock<PendingSync>,
    sync_config: RwLock<SyncConfig>,
    // set_state: // TODO
    report_sender: TracingUnboundedSender<PeerReport>,
    metrics: Option<Metrics>,
}

impl<B: Block> GossipValidator<B> {
    pub(super) fn new(
        config: GossipValidatorConfig,
        // set_state: // TODO
        prometheus_registry: Option<&Registry>,
    ) -> (GossipValidator<B>, TracingUnboundedReceiver<PeerReport>) {
        let metrics: Option<Metrics> = prometheus_registry.and_then(|reg| {
            Metrics::register(reg)
                .map_err(|e| debug!(target: "afa", "Failed to register metrics: {:?}", e))
                .checked_into()
        });

        let sync_config = if config.observer_enabled {
            if config.is_authority {
                SyncConfig::Enabled {
                    only_from_authorities: true,
                }
            } else {
                SyncConfig::Disabled
            }
        } else {
            SyncConfig::Enabled {
                only_from_authorities: false,
            }
        };

        let (tx, rx) = tracing_unbounded("mpsc_aleph_gossip_validator");
        let val = GossipValidator {
            // local_view: RwLock::new(None),
            peers: RwLock::new(Peers::default()),
            authorities: RwLock::new(Vec::new()),
            config: RwLock::new(config),
            next_rebroadcast: RwLock::new(Instant::new() + REBROADCAST_AFTER),
            pending_sync: RwLock::new(PendingSync::None),
            sync_config: RwLock::new(sync_config),
            report_sender: tx,
            metrics,
        };

        (val, rx)
    }
}

impl<B: Block> Validator<B> for GossipValidator<B> {
    fn new_peer(&self, _context: &mut dyn ValidatorContext<B>, who: &PeerId, role: ObservedRole) {
        self.peers.write().insert_peer(who.clone(), role);
    }

    fn peer_disconnected(&self, _context: &mut dyn ValidatorContext<B>, who: &PeerId) {
        self.peers.write().remove_peer(who);
    }

    fn validate(
        &self,
        _context: &mut dyn ValidatorContext<B>,
        _sender: &PeerId,
        _data: &[u8],
    ) -> ValidationResult<B::Hash> {
        todo!()
    }

    fn message_expired(&self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool> {
        Box::new(move |topic, mut data| {
            match GossipMessage::<B>::decode(&mut data) {
                Err(_) => true,
                Ok(GossipMessage::)
            }
        })
    }

    fn message_allowed(&self) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
