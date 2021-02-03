use crate::{
    communication::{
        peer::{
            rep::{PeerGoodBehavior, PeerMisbehavior},
            Peers,
        },
        Epoch,
    },
    nodes::NodeIndex,
    AuthorityId, Sync,
};
use codec::Decode;
use log::debug;
use parking_lot::RwLock;
use prometheus_endpoint::{register, CounterVec, Opts, PrometheusError, Registry, U64};
use sc_network::{ObservedRole, PeerId, ReputationChange};
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};
use sc_telemetry::{telemetry, CONSENSUS_DEBUG};
use sp_core::traits::BareCryptoStorePtr;
use sp_runtime::traits::{Block, CheckedConversion};
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use std::time::{Duration, Instant};

const REBROADCAST_AFTER: Duration = Duration::from_secs(60 * 5);

enum IncomingMessageAction {
    Accept,
    RejectPast,
    RejectFuture,
    RejectOutOfScope,
}

// // TODO
// struct GlobalViewState<N> {}
//
// // TODO
// struct LocalViewState {}

// struct PeerInfo<N> {
//     view: GlobalViewState<N>,
// }

enum MessageAction<H> {
    Keep(H, ReputationChange),
    ProcessAndDiscard(H, ReputationChange),
    Discard(ReputationChange),
}

// TODO
enum PendingSync {
    None,
    Requesting {
        who: PeerId,
        request: SyncRequest,
        instant: Instant,
    },
    Processing {
        instant: Instant,
    },
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

struct SyncRequest {
    epoch: Epoch,
    message: Sync,
}

// TODO
struct SyncResponse {}

// TODO
struct Alert {}

pub(super) enum GossipMessage<H> {
    FetchRequest(FetchRequest),
    FetchResponse(FetchResponse<H>),
    SyncRequest(SyncRequest),
    SyncResponse(SyncResponse),
    Alert(Alert),
}

struct PeerReport {
    who: PeerId,
    change: ReputationChange,
}

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
    peers: RwLock<Peers>,
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

    pub(super) fn report_peer(&self, who: PeerId, change: ReputationChange) {
        self.report_sender
            .unbounded_send(PeerReport { who, change });
    }

    // NOTE: The two `SyncRequest`s may need to be different from each other.
    // I think internally a reformed version can be used.
    fn validate_sync_request(
        &mut self,
        who: &PeerId,
        incoming_request: &SyncRequest,
    ) -> MessageAction<B::Hash> {
        use PendingSync::*;
        match &self.pending_sync {
            Requesting {
                who: peer,
                request,
                instant,
            } => {
                if peer != who {
                    return MessageAction::Discard(PeerMisbehavior::OutOfScopeMessage.cost());
                }

                if request.epoch != incoming_request.epoch {
                    return MessageAction::Discard(PeerMisbehavior::MalformedSync.cost());
                }

                // TODO: I do not know at this point in time if this field will exist. That needs
                // to come from the Aleph protocol.
                // if incoming_request.message.prevotes.is_empty()

                *self.pending_sync.write() = PendingSync::Processing { instant: *instant };

                let topic = super::global_topic::<B>(incoming_request.epoch);
                MessageAction::ProcessAndDiscard(topic, PeerGoodBehavior::ValidatedSync.cost())
            }
            _ => MessageAction::Discard(PeerMisbehavior::OutOfScopeMessage.cost()),
        }
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
        context: &mut dyn ValidatorContext<B>,
        who: &PeerId,
        data: &mut [u8],
    ) -> ValidationResult<B::Hash> {
        // Can fail if the packet is malformed and can't be decoded or the
        // unit's signature is wrong.
        let mut broadcast_topics = Vec::new();
        let mut peer_reply = None;

        let message_name: Option<&str>;

        let action = {
            match GossipMessage::<B>::decode(&mut data) {
                Ok(GossipMessage::SyncRequest(ref message)) => {
                    message_name = Some("sync_request");
                    self.validate_sync_request(who, message)
                }
                Ok(GossipMessage::SyncResponse(ref message)) => {
                    message_name = Some("sync_response");
                    todo!();
                }
                Ok(GossipMessage::FetchRequest(ref message)) => {
                    message_name = Some("fetch_request");
                    todo!();
                }
                Ok(GossipMessage::FetchResponse(ref message)) => {
                    message_name = Some("fetch_response");
                    todo!();
                }
                Ok(GossipMessage::Alert(ref message)) => {
                    message_name = Some("alert");
                    todo!();
                }
                Err(e) => {
                    message_name = None;
                    debug!(target: "afa", "Error decoding message: {}", e.what());
                    telemetry!(CONSENSUS_DEBUG; "afa.err_decoding_msg"; "" => "");

                    let len = std::cmp::min(i32::max_value() as usize, data.len()) as i32;
                    let rep = PeerMisbehavior::UndecodablePacket(len).cost();
                    self.report_peer(who.clone(), rep)
                }
            };
        };

        context.broadcast_message()
    }

    fn message_expired(&self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool> {
        // We do not do anything special if a message expires.
        Box::new(move |_topic, _data| false)
    }

    fn message_allowed(&self) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool> {
        // There should be epoch tracking somewhere. If the data is for a
        // previous epoch, deny.
        Box::new(move |_who, _intent, _topic, _data| true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
