use crate::{communication::{
    peer::{
        rep::{PeerGoodBehavior, PeerMisbehavior, Reputation},
        Peers,
    },
}, temp::{NodeIndex, UnitCoord}, AuthorityId, AuthoritySignature};
use codec::{Decode, Encode};
use log::debug;
use parking_lot::RwLock;
use prometheus_endpoint::{CounterVec, Opts, PrometheusError, Registry, U64};
use sc_network::{ObservedRole, PeerId, ReputationChange};
use sc_network_gossip::{MessageIntent, ValidationResult, Validator, ValidatorContext};
use sc_telemetry::{telemetry, CONSENSUS_DEBUG};
use sp_runtime::traits::Block;
use sp_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use std::{collections::HashSet, marker::PhantomData};
use std::collections::HashMap;
use crate::temp::{Unit, CreatorId};
use sp_application_crypto::RuntimeAppPublic;

#[derive(Debug, Encode, Decode)]
pub(crate) struct SignedUnit<B: Block> {
    unit: Unit<B>,
    signature: AuthoritySignature,
    // NOTE: This will likely be changed to a usize to get the authority out of
    // a map in the future to reduce data sizes of packets.
    id: AuthorityId,
}

impl<B: Block> SignedUnit<B> {
    pub(crate) fn encode_unit_with_buffer(&self, buf: &mut Vec<u8>) {
        buf.clear();
        self.unit.encode_to(buf);
    }

    pub fn verify_unit_signature_with_buffer(&self, buf: &mut Vec<u8>) -> bool {
        self.encode_unit_with_buffer(buf);

        let valid = self.id.verify(&buf, &self.signature);
        if !valid {
            debug!(target: "afa", "Bad signature message from {:?}", self.unit.creator);
        }

        valid
    }

    pub fn verify_unit_signature(&self) -> bool {
        self.verify_unit_signature_with_buffer(&mut Vec::new())
    }
}

#[derive(Debug)]
enum MessageAction<H> {
    Keep(H, Reputation),
    ProcessAndDiscard(H, Reputation),
    Discard(Reputation),
}

#[derive(Debug, Encode, Decode)]
pub(crate) struct Multicast<B: Block> {
    signed_unit: SignedUnit<B>,
}

#[derive(Debug, Encode, Decode, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FetchRequest {
    coords: Vec<UnitCoord>,
    peer_id: NodeIndex,
}

#[derive(Debug, Encode, Decode)]
pub(crate) struct FetchResponse<B: Block> {
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

type PrometheusResult<T> = Result<T, PrometheusError>;

struct Metrics {
    messages_validated: CounterVec<U64>,
}

impl Metrics {
    pub(crate) fn register(registry: &prometheus_endpoint::Registry) -> PrometheusResult<Self> {
        Ok(Self {
            messages_validated: prometheus_endpoint::register(
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

type PeerIdBytes = Vec<u8>;

pub(super) struct GossipValidator<B: Block> {
    peers: RwLock<Peers>,
    authorities: RwLock<HashSet<AuthorityId>>,
    report_sender: TracingUnboundedSender<PeerReport>,
    metrics: Option<Metrics>,
    pending_requests: RwLock<HashSet<(PeerIdBytes, Vec<UnitCoord>)>>,
    phantom: PhantomData<B>,
}

impl<B: Block> GossipValidator<B> {
    pub(crate) fn new(
        prometheus_registry: Option<&Registry>,
    ) -> (GossipValidator<B>, TracingUnboundedReceiver<PeerReport>) {
        let metrics: Option<Metrics> = prometheus_registry.and_then(|reg| {
            Metrics::register(reg)
                .map_err(|e| debug!(target: "afa", "Failed to register metrics: {:?}", e))
                .ok()
        });

        let (tx, rx) = tracing_unbounded("mpsc_aleph_gossip_validator");
        let val = GossipValidator {
            peers: RwLock::new(Peers::default()),
            authorities: RwLock::new(HashSet::new()),
            report_sender: tx,
            metrics,
            pending_requests: RwLock::new(HashSet::new()),
            phantom: PhantomData::default(),
        };

        (val, rx)
    }

    pub(crate) fn report_peer(&self, who: PeerId, change: ReputationChange) {
        let _ = self
            .report_sender
            .unbounded_send(PeerReport { who, change });
    }

    pub(crate) fn note_pending_fetch_request(&mut self, who: PeerId, mut request: FetchRequest) {
        let mut pending_request = self.pending_requests.write();
        request.coords.sort();
        pending_request.insert((who.into_bytes(), request.coords));
    }

    pub(crate) fn set_authorities<I>(&mut self, authorities: I)
    where
        I: IntoIterator<Item = AuthorityId>,
    {
        let mut old_authorities = self.authorities.write();
        old_authorities.clear();
        old_authorities.extend(authorities.into_iter());
    }

    pub(crate) fn remove_authority(&mut self, authority: &AuthorityId) {
        let mut authorities = self.authorities.write();
        authorities.remove(authority);
    }

    fn validate_signed_unit(
        &self,
        sender: &PeerId,
        signed_unit: &SignedUnit<B>,
    ) -> Result<(), MessageAction<B::Hash>> {
        let id = &signed_unit.id;
        if !self.authorities.read().contains(id) {
            debug!(target: "afa", "Message from unknown authority: {} from {}", id, sender);
            return Err(MessageAction::Discard(PeerMisbehavior::UnknownVoter.into()));
        }

        if !signed_unit.verify_unit_signature() {
            debug!(target: "afa", "Bad message signature: {} from {}", id, sender);
            return Err(MessageAction::Discard(PeerMisbehavior::BadSignature.into()));
        }

        Ok(())
    }

    fn validate_multicast(
        &self,
        sender: &PeerId,
        message: &Multicast<B>,
    ) -> MessageAction<B::Hash> {
        match self.validate_signed_unit(sender, &message.signed_unit) {
            Ok(_) => {
                let topic: <B as Block>::Hash = super::multicast_topic::<B>(
                    message.signed_unit.unit.round,
                    message.signed_unit.unit.epoch_id,
                );
                MessageAction::Keep(topic, PeerGoodBehavior::Multicast.into())
            }
            Err(e) => e,
        }
    }

    fn validate_fetch_response(
        &self,
        sender: &PeerId,
        message: &FetchResponse<B>,
    ) -> MessageAction<B::Hash> {
        let mut pending_requests = self.pending_requests.write();
        if pending_requests.len() != 0 {
            let mut coords = Vec::with_capacity(message.signed_units.len());
            for signed_unit in &message.signed_units {
                let unit = &signed_unit.unit;
                let coord: UnitCoord = unit.into();
                coords.push(coord);
            }
            coords.sort();

            if !pending_requests.remove(&(sender.clone().into_bytes(), coords)) {
                return MessageAction::Discard(Reputation::from(PeerMisbehavior::OutOfScopeResponse));
            }

            for signed_unit in &message.signed_units {
                if let Err(e) = self.validate_signed_unit(sender, signed_unit) {
                    return e;
                }
            }

            let topic: <B as Block>::Hash = super::index_topic::<B>(message.peer_id);
            MessageAction::Keep(topic, PeerGoodBehavior::FetchResponse.into())
        } else {
            MessageAction::Discard(Reputation::from(PeerMisbehavior::OutOfScopeResponse))
        }
    }

    // TODO: Rate limiting should be applied here. We would not want to let an unlimited amount of
    // requests. Though, it should be checked if this is already done on the other layers. Not to
    // my knowledge though.
    fn validate_fetch_request(
        &self,
        _sender: &PeerId,
        message: &FetchRequest,
    ) -> MessageAction<B::Hash> {
        let topic: <B as Block>::Hash = super::index_topic::<B>(message.peer_id);

        MessageAction::ProcessAndDiscard(topic, PeerGoodBehavior::FetchRequest.into())
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
        let message_name: Option<&str>;
        let action = match GossipMessage::<B>::decode(&mut data) {
            Ok(GossipMessage::Multicast(ref message)) => {
                message_name = Some("multicast");
                self.validate_multicast(sender, message)
            }
            Ok(GossipMessage::FetchRequest(ref message)) => {
                message_name = Some("fetch_request");
                self.validate_fetch_request(sender, message)
            }
            Ok(GossipMessage::FetchResponse(ref message)) => {
                message_name = Some("fetch_response");
                self.validate_fetch_response(sender, message)
            }
            Ok(GossipMessage::Alert(ref _message)) => {
                message_name = Some("fetch_response");
                todo!()
            }
            Err(e) => {
                message_name = None;
                debug!(target: "afa", "Error decoding message: {}", e.what());
                telemetry!(CONSENSUS_DEBUG; "afa.err_decoding_msg"; "" => "");

                let len = std::cmp::min(i32::max_value() as usize, data.len()) as i32;
                MessageAction::Discard(PeerMisbehavior::UndecodablePacket(len).into())
            }
        };

        if let (Some(metrics), Some(message_name)) = (&self.metrics, message_name) {
            let action_name = match action {
                MessageAction::Keep(_, _) => "keep",
                MessageAction::ProcessAndDiscard(_, _) => "process_and_discard",
                MessageAction::Discard(_) => "discard",
            };
            metrics
                .messages_validated
                .with_label_values(&[message_name, action_name])
                .inc();
        }

        match action {
            MessageAction::Keep(topic, rep_change) => {
                self.report_peer(sender.clone(), rep_change.change());
                context.broadcast_message(topic, data.to_vec(), false);
                ValidationResult::ProcessAndKeep(topic)
            }
            MessageAction::ProcessAndDiscard(topic, rep_change) => {
                self.report_peer(sender.clone(), rep_change.change());
                ValidationResult::ProcessAndDiscard(topic)
            }
            MessageAction::Discard(rep_change) => {
                self.report_peer(sender.clone(), rep_change.change());
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
    use crate::{
        temp::{ControlHash, CreatorId, EpochId, NodeMap, Round, Unit},
        AuthorityPair, AuthoritySignature,
    };
    use sp_core::{Pair, H256};
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

    fn new_gossip_validator() -> GossipValidator<Block> {
        GossipValidator::<Block>::new(None).0
    }

    fn new_control_hash() -> ControlHash<Hash> {
        ControlHash {
            parents: NodeMap(vec![false]),
            hash: Hash::from([1u8; 32]),
        }
    }

    fn new_unit() -> Unit<Block> {
        let control_hash = new_control_hash();
        Unit {
            creator: CreatorId(0),
            round: Round(0),
            epoch_id: EpochId(0),
            hash: Hash::from([1u8; 32]),
            control_hash,
            best_block: Hash::from([1u8; 32]),
        }
    }

    fn new_signed_unit(unit: Unit<Block>, signature: AuthoritySignature, id: AuthorityId) -> SignedUnit<Block> {
        SignedUnit {
            unit,
            signature,
            id
        }
    }

    fn new_multicast(
        unit: Unit<Block>,
        signature: AuthoritySignature,
        id: AuthorityId,
    ) -> Multicast<Block> {
        Multicast {
            signed_unit: new_signed_unit(unit, signature, id),
        }
    }

    #[test]
    fn good_multicast() {
        let mut val = new_gossip_validator();
        let keypair = AuthorityPair::from_seed_slice(&[1u8; 32]).unwrap();
        let unit = new_unit();
        let signature = keypair.sign(&unit.encode());
        let message: Multicast<Block> = new_multicast(unit, signature, keypair.public());
        let peer = PeerId::random();
        val.set_authorities(vec![keypair.public()]);
        val.peers
            .write()
            .insert_peer(peer.clone(), ObservedRole::Authority);
        let res = val.validate_multicast(&peer, &message);

        assert!(matches!(res, MessageAction::Keep(..)));
    }

    #[test]
    fn bad_signature_multicast() {
        let mut val = new_gossip_validator();
        let keypair = AuthorityPair::from_seed_slice(&[1u8; 32]).unwrap();
        let unit = new_unit();
        let signature = AuthoritySignature::default();
        let message: Multicast<Block> = new_multicast(unit, signature, keypair.public());
        let peer = PeerId::random();
        val.set_authorities(vec![keypair.public()]);
        val.peers
            .write()
            .insert_peer(peer.clone(), ObservedRole::Authority);

        let res = val.validate_multicast(&peer, &message);
        let _action: MessageAction<Hash> =
            MessageAction::Discard(PeerMisbehavior::BadSignature.into());
        assert!(matches!(res, _action));
    }

    #[test]
    fn unknown_authority_multicast() {
        let mut val = new_gossip_validator();
        let keypair = AuthorityPair::from_seed_slice(&[1u8; 32]).unwrap();
        let unit = new_unit();
        let signature = keypair.sign(&unit.encode());
        let message: Multicast<Block> = new_multicast(unit, signature, keypair.public());
        let peer = PeerId::random();
        val.set_authorities(vec![AuthorityId::default()]);
        val.peers
            .write()
            .insert_peer(peer.clone(), ObservedRole::Authority);

        let res = val.validate_multicast(&peer, &message);
        let _action: MessageAction<Hash> =
            MessageAction::Discard(PeerMisbehavior::UnknownVoter.into());
        assert!(matches!(res, _action));
    }

    #[test]
    fn good_fetch_response() {
        let mut val = new_gossip_validator();
        let keypair = AuthorityPair::from_seed_slice(&[1u8; 32]).unwrap();

        let mut coords: Vec<UnitCoord> = Vec::with_capacity(10);
        for x in 0..10 {
            let unit_coord = UnitCoord {
                creator: CreatorId(x),
                round: Round(x + 1),
            };
            coords.push(unit_coord);
        }

        let peer = PeerId::random();
        let fetch_request = FetchRequest {
            coords,
            peer_id: NodeIndex(0),
        };
        val.note_pending_fetch_request(peer.clone(), fetch_request);

        val.set_authorities(vec![keypair.public()]);
        val.peers.write().insert_peer(peer.clone(), ObservedRole::Authority);

        let mut signed_units = Vec::with_capacity(10);
        for x in 0..10 {
            let unit: Unit<Block> = Unit {
                creator: CreatorId(x),
                round: Round(x + 1),
                epoch_id: EpochId(0),
                hash: Hash::from([1u8; 32]),
                control_hash: new_control_hash(),
                best_block: Hash::from([1u8; 32]),
            };
            let signature = keypair.sign(&unit.encode());

            let signed_unit = SignedUnit {
                unit,
                signature,
                id: keypair.public(),
            };

            signed_units.push(signed_unit);
        }

        let fetch_response = FetchResponse {
            signed_units,
            peer_id: Default::default()
        };

        let res = val.validate_fetch_response(&peer, &fetch_response);
        assert!(matches!(res, MessageAction::Keep(..)))
    }

    #[test]
    fn bad_signature_fetch_response() {
        let mut val = new_gossip_validator();

        let mut coords: Vec<UnitCoord> = Vec::with_capacity(10);
        for x in 0..10 {
            let unit_coord = UnitCoord {
                creator: CreatorId(x),
                round: Round(x + 1),
            };
            coords.push(unit_coord);
        }

        let peer = PeerId::random();
        let fetch_request = FetchRequest {
            coords,
            peer_id: NodeIndex(0),
        };
        val.note_pending_fetch_request(peer.clone(), fetch_request);

        val.set_authorities(vec![AuthorityId::default()]);
        val.peers.write().insert_peer(peer.clone(), ObservedRole::Authority);

        let mut signed_units = Vec::with_capacity(10);
        for x in 0..10 {
            let unit: Unit<Block> = Unit {
                creator: CreatorId(x),
                round: Round(x + 1),
                epoch_id: EpochId(0),
                hash: Hash::from([1u8; 32]),
                control_hash: new_control_hash(),
                best_block: Hash::from([1u8; 32]),
            };
            let signature = AuthoritySignature::default();

            let signed_unit = SignedUnit {
                unit,
                signature,
                id: AuthorityId::default(),
            };

            signed_units.push(signed_unit);
        }

        let fetch_response = FetchResponse {
            signed_units,
            peer_id: Default::default()
        };

        let res = val.validate_fetch_response(&peer, &fetch_response);
        let _action: MessageAction<Hash> =
            MessageAction::Discard(PeerMisbehavior::BadSignature.into());
        assert!(matches!(res, _action));
    }

    #[test]
    fn unknown_authority_fetch_response() {
        let mut val = new_gossip_validator();
        let keypair = AuthorityPair::from_seed_slice(&[1u8; 32]).unwrap();

        let mut coords: Vec<UnitCoord> = Vec::with_capacity(10);
        for x in 0..10 {
            let unit_coord = UnitCoord {
                creator: CreatorId(x),
                round: Round(x + 1),
            };
            coords.push(unit_coord);
        }

        let peer = PeerId::random();
        let fetch_request = FetchRequest {
            coords,
            peer_id: NodeIndex(0),
        };
        val.note_pending_fetch_request(peer.clone(), fetch_request);

        val.peers.write().insert_peer(peer.clone(), ObservedRole::Authority);

        let mut signed_units = Vec::with_capacity(10);
        for x in 0..10 {
            let unit: Unit<Block> = Unit {
                creator: CreatorId(x),
                round: Round(x + 1),
                epoch_id: EpochId(0),
                hash: Hash::from([1u8; 32]),
                control_hash: new_control_hash(),
                best_block: Hash::from([1u8; 32]),
            };
            let signature = keypair.sign(&unit.encode());

            let signed_unit = SignedUnit {
                unit,
                signature,
                id: keypair.public(),
            };

            signed_units.push(signed_unit);
        }

        let fetch_response = FetchResponse {
            signed_units,
            peer_id: Default::default()
        };

        let res = val.validate_fetch_response(&peer, &fetch_response);
        let _action: MessageAction<Hash> =
            MessageAction::Discard(PeerMisbehavior::UnknownVoter.into());
        assert!(matches!(res, _action));
    }

    // TODO: Once the fetch request has a bit more logic in it, there needs to
    // be a test for it.
}
