use crate::{
    communication::{
        peer::{
            rep::{PeerGoodBehavior, PeerMisbehavior},
            Peers,
        },
        SignedUnit,
    },
    temp::{NodeIndex, UnitCoord},
    AuthorityId,
};
use codec::{Decode, Encode};
use log::debug;
use parking_lot::RwLock;
use prometheus_endpoint::Registry;
use sc_network::{ObservedRole, PeerId, ReputationChange};
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};
use sc_telemetry::{telemetry, CONSENSUS_DEBUG};
use sp_runtime::traits::Block;
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use std::marker::PhantomData;

#[derive(Debug)]
enum MessageAction<H> {
    Keep(H, ReputationChange),
    ProcessAndDiscard(H, ReputationChange),
    Discard(ReputationChange),
}

// TODO
#[derive(Debug, Encode, Decode)]
struct Multicast<B: Block> {
    signed_unit: SignedUnit<B>,
}

#[derive(Debug, Encode, Decode)]
struct FetchRequest {
    coords: Vec<UnitCoord>,
    peer_id: NodeIndex,
}

#[derive(Debug, Encode, Decode)]
struct FetchResponse<B: Block> {
    signed_units: Vec<SignedUnit<B>>,
    peer_id: NodeIndex,
}

// TODO
#[derive(Debug, Encode, Decode)]
struct Alert {}

#[derive(Debug, Encode, Decode)]
enum GossipMessage<B: Block> {
    Multicast(Multicast<B>),
    FetchRequest(FetchRequest),
    FetchResponse(FetchResponse<B>),
    Alert(Alert),
}

pub(crate) struct PeerReport {
    who: PeerId,
    change: ReputationChange,
}

// type PrometheusResult<T> = Result<T, PrometheusError>;

// struct Metrics {
//     messages_validated: CounterVec<U64>,
// }

// impl Metrics {
//     pub(crate) fn register(registry: &prometheus_endpoint::Registry) -> PrometheusResult<Self> {
//         Ok(Self {
//             messages_validated: register(
//                 CounterVec::new(
//                     Opts::new(
//                         "finality_aleph_communication_gossip_validator_messages",
//                         "Number of messages validated by the finality aleph gossip validator.",
//                     ),
//                     &["message", "action"],
//                 )?,
//                 registry,
//             )?,
//         })
//     }
// }

pub(super) struct GossipValidator<B: Block> {
    peers: RwLock<Peers>,
    authorities: RwLock<Vec<AuthorityId>>,
    // config: RwLock<GossipValidatorConfig>,
    // epoch: EpochId,
    report_sender: TracingUnboundedSender<PeerReport>,
    // metrics: Option<Metrics>,
    phantom: PhantomData<B>,
}

impl<B: Block> GossipValidator<B> {
    pub(crate) fn new(
        // config: GossipValidatorConfig,
        // epoch: EpochId,
        _prometheus_registry: Option<&Registry>,
    ) -> (GossipValidator<B>, TracingUnboundedReceiver<PeerReport>) {
        // let metrics: Option<Metrics> = prometheus_registry.and_then(|reg| {
        //     Metrics::register(reg)
        //         .map_err(|e| debug!(target: "afa", "Failed to register metrics: {:?}", e))
        //         .checked_into()
        // });

        let (tx, rx) = tracing_unbounded("mpsc_aleph_gossip_validator");
        let val = GossipValidator {
            peers: RwLock::new(Peers::default()),
            authorities: RwLock::new(Vec::new()),
            // config: RwLock::new(config),
            // epoch,
            report_sender: tx,
            // metrics,
            phantom: PhantomData::default(),
        };

        (val, rx)
    }

    pub(crate) fn report_peer(&self, who: PeerId, change: ReputationChange) {
        let _ = self
            .report_sender
            .unbounded_send(PeerReport { who, change });
    }

    fn validate_ch_unit(
        &self,
        sender: &PeerId,
        signed_ch_unit: &SignedUnit<B>,
    ) -> Result<(), MessageAction<B::Hash>> {
        let id = &signed_ch_unit.id;
        if !self.authorities.read().contains(id) {
            debug!(target: "afa", "Message from unknown authority: {} from {}", id, sender);
            // TODO telemetry
            // telemetry!(CONSENSUS_DEBUG, "afa.bad_msg_signature"; "sig")
            return Err(MessageAction::Discard(PeerMisbehavior::UnknownVoter.cost()));
        }

        if !super::verify_unit_signature(
            &signed_ch_unit.unit,
            &signed_ch_unit.signature,
            &signed_ch_unit.id,
        ) {
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
                let topic: <B as Block>::Hash = super::multicast_topic::<B>(
                    message.signed_unit.unit.round,
                    message.signed_unit.unit.epoch_id,
                );
                MessageAction::Keep(topic, PeerGoodBehavior::GoodMulticast.benefit())
            }
            Err(e) => e,
        }
    }

    fn validate_fetch_response(
        &self,
        sender: &PeerId,
        message: &FetchResponse<B>,
    ) -> MessageAction<B::Hash> {
        for signed_ch_unit in &message.signed_units {
            if let Err(e) = self.validate_ch_unit(sender, signed_ch_unit) {
                return e;
            }
        }
        let topic: <B as Block>::Hash = super::index_topic::<B>(message.peer_id);

        MessageAction::ProcessAndDiscard(topic, PeerGoodBehavior::ValidatedSync.benefit())
    }

    fn validate_fetch_request(
        &self,
        _sender: &PeerId,
        message: &FetchRequest,
    ) -> MessageAction<B::Hash> {
        let topic: <B as Block>::Hash = super::index_topic::<B>(message.peer_id);

        MessageAction::ProcessAndDiscard(topic, PeerGoodBehavior::GoodFetchRequest.benefit())
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
        // let message_name: Option<&str>;
        let action = match GossipMessage::<B>::decode(&mut data) {
            Ok(GossipMessage::Multicast(ref message)) => {
                // message_name = Some("multicast");
                self.validate_multicast(sender, message)
            }
            Ok(GossipMessage::FetchRequest(ref message)) => {
                // message_name = Some("fetch_request");
                self.validate_fetch_request(sender, message)
            }
            Ok(GossipMessage::FetchResponse(ref message)) => {
                // message_name = Some("fetch_response");
                self.validate_fetch_response(sender, message)
            }
            Ok(GossipMessage::Alert(ref _message)) => {
                // message_name = Some("alert");
                todo!()
            }
            Err(e) => {
                // message_name = None;
                debug!(target: "afa", "Error decoding message: {}", e.what());
                telemetry!(CONSENSUS_DEBUG; "afa.err_decoding_msg"; "" => "");

                let len = std::cmp::min(i32::max_value() as usize, data.len()) as i32;
                let rep = PeerMisbehavior::UndecodablePacket(len).cost();
                MessageAction::Discard(rep)
            }
        };

        match action {
            MessageAction::Keep(topic, cb) => {
                self.report_peer(sender.clone(), cb);
                context.broadcast_message(topic, data.to_vec(), false);
                ValidationResult::ProcessAndKeep(topic)
            }
            MessageAction::ProcessAndDiscard(topic, cb) => {
                self.report_peer(sender.clone(), cb);
                ValidationResult::ProcessAndDiscard(topic)
            }
            MessageAction::Discard(cb) => {
                self.report_peer(sender.clone(), cb);
                ValidationResult::Discard
            }
        }
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
    use crate::temp::{ControlHash, CreatorId, NodeMap, Round, Unit};
    use sp_core::{Public, H256};
    use sp_runtime::traits::Extrinsic as ExtrinsicT;

    #[derive(Debug, PartialEq, Clone, Eq, Encode, Decode, serde::Serialize)]
    pub struct Extrinsic {}

    parity_util_mem::malloc_size_of_is_0!(Extrinsic);

    impl ExtrinsicT for Extrinsic {
        type Call = Extrinsic;
        type SignaturePayload = ();
    }

    pub type BlockNumber = u64;

    pub type Hashing = sp_runtime::traits::BlakeTwo256;

    pub type Header = sp_runtime::generic::Header<BlockNumber, Hashing>;

    pub type Hash = H256;

    pub type Block = sp_runtime::generic::Block<Header, Extrinsic>;

    #[test]
    fn multicast_message() {
        let (val, _) = GossipValidator::<Hash>::new(None);
        let control_hash: ControlHash<Hash> = ControlHash {
            parents: NodeMap(vec![false]),
            hash: H256::from([1u8; 32]),
        };
        let unit = Unit {
            creator: CreatorId(0),
            round: Round(0),
            epoch_id: EpochId(0),
            hash: H256::from([1u8; 32]),
            control_hash,
            best_block: H256::from([1u8; 32]),
        };
        let message = Multicast {
            signed_unit: SignedUnit {
                unit,
                signature: Default::default(),
                id: AuthorityId::from_slice(&[1u8; 32]),
            },
        };
        let peer = PeerId::random();
        val.peers
            .write()
            .insert_peer(peer.clone(), ObservedRole::Authority);
        let res = val.validate_multicast(&peer, &message);
        println!("{:?}", res);
    }
}
