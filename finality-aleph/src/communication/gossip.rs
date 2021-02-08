use crate::{
    communication::{
        peer::{
            rep::{PeerGoodBehavior, PeerMisbehavior},
            Peers,
        },
        SignedCHUnit,
    },
    nodes::NodeIndex,
    AuthorityId, AuthoritySignature, CHUnit, EpochId,
};
use codec::{Decode, Encode};
use log::debug;
use parking_lot::RwLock;
use prometheus_endpoint::{register, CounterVec, Opts, PrometheusError, Registry, U64};
use sc_network::{ObservedRole, PeerId, ReputationChange};
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};
use sc_telemetry::{telemetry, CONSENSUS_DEBUG};
use sp_core::traits::BareCryptoStorePtr;
use sp_runtime::traits::{Block, Hash};
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use std::{
    marker::PhantomData,
    time::{Duration, Instant},
};

const REBROADCAST_AFTER: Duration = Duration::from_secs(60 * 5);

enum IncomingMessageAction {
    Accept,
    RejectPast,
    RejectFuture,
    RejectOutOfScope,
}

enum MessageAction<H> {
    Keep(H, ReputationChange),
    ProcessAndDiscard(ReputationChange),
    Discard(ReputationChange),
}

// // TODO
// enum PendingSync {
//     None,
//     Requesting {
//         who: PeerId,
//         request: SyncRequest,
//         instant: Instant,
//     },
//     Processing {
//         instant: Instant,
//     },
// }
//
// enum SyncConfig {
//     Enabled { only_from_authorities: bool },
//     Disabled,
// }

// TODO
#[derive(Debug, Encode, Decode)]
struct Multicast<B: Block> {
    signed_unit: SignedCHUnit<B>,
}

#[derive(Debug, Encode, Decode)]
struct FetchRequest {
    hashes: Vec<NodeIndex>,
    unit_id: NodeIndex,
}

#[derive(Debug, Encode, Decode)]
struct FetchResponse<B: Block> {
    signed_units: Vec<SignedCHUnit<B>>,
    unit_id: NodeIndex,
}

// struct SyncRequest {
//     epoch_id: EpochId,
// }

// // TODO
// struct SyncResponse {
//     epoch_id: EpochId,
// }

// TODO
#[derive(Debug, Encode, Decode)]
struct Alert {}

// #[derive(Debug, Encode, Decode)]
// pub(crate) struct NeighborPacketV1 {
//     pub epoch_id: EpochId,
// }

// #[derive(Debug, Encode, Decode)]
// pub(crate) enum VersionedNeighborPacket {
//     V1(NeighborPacketV1),
// }
//
// // If multiple versions, should be improved with TryFrom
// impl From<VersionedNeighborPacket> for NeighborPacketV1 {
//     fn from(packet: VersionedNeighborPacket) -> Self {
//         match packet {
//             VersionedNeighborPacket::V1(p) => p,
//         }
//     }
// }

#[derive(Debug, Encode, Decode)]
pub(super) enum GossipMessage<B: Block> {
    Multicast(Multicast<B>),
    FetchRequest(FetchRequest),
    FetchResponse(FetchResponse<B>),
    // Neighbor(VersionedNeighborPacket),
    // SyncRequest(SyncRequest),
    // SyncResponse(SyncResponse),
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

pub struct GossipValidatorConfig {
    pub gossip_duration: Duration,
    pub justification_period: u32,
    pub observer_enabled: bool,
    pub is_authority: bool,
    pub name: Option<String>,
    pub keystore: Option<BareCryptoStorePtr>,
}

pub(super) struct GossipValidator<B: Block> {
    peers: RwLock<Peers>,
    // live_topics // TODO
    authorities: RwLock<Vec<AuthorityId>>,
    config: RwLock<GossipValidatorConfig>,
    next_rebroadcast: RwLock<Instant>,
    // pending_sync: RwLock<PendingSync>,
    // sync_config: RwLock<SyncConfig>,
    epoch: EpochId,
    report_sender: TracingUnboundedSender<PeerReport>,
    // TODO
    // metrics: Option<Metrics>,
    // TEMP
    phantom: PhantomData<B>,
}

impl<B: Block> GossipValidator<B> {
    pub(super) fn new(
        config: GossipValidatorConfig,
        epoch: EpochId,
        _prometheus_registry: Option<&Registry>,
    ) -> (GossipValidator<B>, TracingUnboundedReceiver<PeerReport>) {
        // let metrics: Option<Metrics> = prometheus_registry.and_then(|reg| {
        //     Metrics::register(reg)
        //         .map_err(|e| debug!(target: "afa", "Failed to register metrics: {:?}", e))
        //         .checked_into()
        // });

        // let sync_config = if config.observer_enabled {
        //     if config.is_authority {
        //         SyncConfig::Enabled {
        //             only_from_authorities: true,
        //         }
        //     } else {
        //         SyncConfig::Disabled
        //     }
        // } else {
        //     SyncConfig::Enabled {
        //         only_from_authorities: false,
        //     }
        // };

        let (tx, rx) = tracing_unbounded("mpsc_aleph_gossip_validator");
        let val = GossipValidator {
            peers: RwLock::new(Peers::default()),
            authorities: RwLock::new(Vec::new()),
            config: RwLock::new(config),
            next_rebroadcast: RwLock::new(Instant::now() + REBROADCAST_AFTER),
            // pending_sync: RwLock::new(PendingSync::None),
            // sync_config: RwLock::new(sync_config),
            epoch,
            report_sender: tx,
            // metrics,
            phantom: PhantomData::default(),
        };

        (val, rx)
    }

    pub(super) fn report_peer(&self, who: PeerId, change: ReputationChange) {
        self.report_sender
            .unbounded_send(PeerReport { who, change });
    }

    // // NOTE: The two `SyncRequest`s may need to be different from each other.
    // // I think internally a reformed version can be used.
    // // NOTE: This is kind of weird and I don't understand why with Grandpa it
    // // throws away every other request. Maybe it will become more clear once
    // // I add more to it.
    // // It goes from note_catch_up
    // fn validate_sync_request(
    //     &mut self,
    //     who: &PeerId,
    //     incoming_request: &SyncRequest,
    // ) -> MessageAction<B::Hash> {
    //     use PendingSync::*;
    //     match &self.pending_sync {
    //         Requesting {
    //             who: peer,
    //             request,
    //             instant,
    //         } => {
    //             if peer != who {
    //                 return MessageAction::Discard(PeerMisbehavior::OutOfScopeMessage.cost());
    //             }
    //
    //             if request.epoch != incoming_request.epoch {
    //                 return MessageAction::Discard(PeerMisbehavior::MalformedSync.cost());
    //             }
    //
    //             *self.pending_sync.write() = PendingSync::Processing { instant: *instant };
    //
    //             let topic = super::global_topic::<B>(incoming_request.epoch);
    //             MessageAction::ProcessAndDiscard(topic, PeerGoodBehavior::ValidatedSync.cost())
    //         }
    //         _ => MessageAction::Discard(PeerMisbehavior::OutOfScopeMessage.cost()),
    //     }
    // }

    // fn import_neighbor_message(&self, sender: &PeerId, packet: NeighborPacketV1) {
    //     let update_res = self.peers.write().update_peer(sender, packet);
    //
    //     // let (cost_benefit)
    // }

    fn validate_ch_unit(
        &self,
        sender: &PeerId,
        signed_ch_unit: &SignedCHUnit<B>,
    ) -> Result<(), MessageAction<B::Hash>> {
        let id = &signed_ch_unit.id;
        if !self.authorities.read().contains(id) {
            debug!(target: "afa", "Message from unknown authority: {} from {}", id, sender);
            // TODO telemetry
            // telemetry!(CONSENSUS_DEBUG, "afa.bad_msg_signature"; "sig")
            return Err(MessageAction::Discard(PeerMisbehavior::UnknownVoter.cost()));
        }

        if !super::verify_unit_signature(&signed_ch_unit) {
            debug!(target: "afa", "Bad message signature: {} from {}", id, sender);
            // TODO telemetry
            return Err(MessageAction::Discard(PeerMisbehavior::BadSignature.cost()));
        }

        Ok(())
    }

    fn validate_multicast(
        &self,
        sender: &PeerId,
        message: &Multicast<B>,
    ) -> MessageAction<B::Hash> {
        match self.validate_ch_unit(sender, &message.signed_unit) {
            Ok(_) => {
                let topic = super::topic(
                    message.signed_unit.unit.round(),
                    message.signed_unit.unit.epoch(),
                );
                MessageAction::Keep(topic, PeerGoodBehavior::GoodMulticast.benefit())
            }
            Err(e) => e,
        }
    }

    fn validate_fetch_response<H: Hash>(
        &self,
        sender: &PeerId,
        message: &FetchResponse<B>,
    ) -> MessageAction<B::Hash> {
        for signed_ch_unit in &message.signed_units {
            if let Err(e) = self.validate_ch_unit(sender, signed_ch_unit) {
                return e;
            }
        }

        MessageAction::ProcessAndDiscard(PeerGoodBehavior::ValidatedSync.benefit())
    }

    fn validate_fetch_request(
        &self,
        _sender: &PeerId,
        _message: &FetchRequest,
    ) -> MessageAction<B::Hash> {
        MessageAction::ProcessAndDiscard(PeerGoodBehavior::GoodFetchRequest.benefit())
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
        sender: &PeerId,
        mut data: &[u8],
    ) -> ValidationResult<B::Hash> {
        // Can fail if the packet is malformed and can't be decoded or the
        // unit's signature is wrong.
        let mut broadcast_topics = Vec::new();
        let mut peer_reply = None;

        let message_name: Option<&str>;

        let action = {
            match GossipMessage::<B>::decode(&mut data) {
                Ok(GossipMessage::Multicast(ref message)) => {
                    message_name = Some("multicast");
                    self.validate_multicast(sender, message)
                }
                // Ok(GossipMessage::Neighbor(update)) => {
                //     message_name = Some("neighbor");
                //     // self.import_neighbor_message(sender, update.into())
                // }
                Ok(GossipMessage::FetchRequest(ref message)) => {
                    message_name = Some("fetch_request");
                    self.validate_fetch_request(sender, message)
                }
                Ok(GossipMessage::FetchResponse(ref message)) => {
                    message_name = Some("fetch_response");
                    self.validate_fetch_response(sender, message)
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
                    self.report_peer(sender.clone(), rep);
                }
            };
        };

        // context.broadcast_message()
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
