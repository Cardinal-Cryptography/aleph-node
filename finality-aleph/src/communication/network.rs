use crate::{communication::{
    epoch_topic,
    gossip::{FetchRequest, GossipMessage, GossipValidator, Multicast, PeerReport, SignedUnit},
}, config::Config, hash::Hash, AuthorityCryptoStore, UnitCoord};
use codec::{Encode, Decode};
use futures::{channel::mpsc, prelude::*};
use log::{trace, debug};
use parking_lot::Mutex;
use prometheus_endpoint::Registry;
use rush::{EpochId, NotificationIn, NotificationOut};
use sc_network::PeerId;
use sc_network_gossip::{GossipEngine, Network as GossipNetwork};
use sp_runtime::{traits::Block, ConsensusEngineId};
use sp_utils::mpsc::TracingUnboundedReceiver;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// Name of the notifications protocol used by Aleph Zero. This is how messages
/// are subscribed to to ensure that we are gossiping and communicating with our
/// own network.
pub(crate) const ALEPH_PROTOCOL_NAME: &str = "/cardinals/aleph/1";

pub(crate) const ALEPH_ENGINE_ID: ConsensusEngineId = *b"ALPH";

pub(crate) trait Network<B: Block>: GossipNetwork<B> + Clone + Send + 'static {}

struct NotificationOuts<B: Block, H: Hash> {
    network: Arc<Mutex<GossipEngine<B>>>,
    gossip_validator: Arc<GossipValidator<B, H>>,
    sender: mpsc::Sender<GossipMessage<B, H>>,
    epoch_id: EpochId,
    auth_cryptostore: AuthorityCryptoStore,
}

impl<B: Block, H: Hash> Sink<NotificationOut<H, B::Hash>> for NotificationOuts<B, H> {
    // TODO! error
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: Pin<&mut Self>,
        item: NotificationOut<H, B::Hash>,
    ) -> Result<(), Self::Error> {
        return match item {
            NotificationOut::CreatedUnit(u) => {
                let signed_unit = match super::gossip::sign_unit(&self.auth_cryptostore, u) {
                    Some(s) => s,
                    None => return Err(()),
                };
                let message = GossipMessage::Multicast(Multicast {
                    signed_unit: signed_unit.clone(),
                });

                let topic: <B as Block>::Hash = super::epoch_topic::<B>(self.epoch_id);

                self.network.lock().gossip_message(topic, message.encode(), false);

                self.sender.start_send(message).map_err(|_e| ())
            }
            NotificationOut::MissingUnits(coords, aux) => {
                let n_coords = {
                    let mut n_coords: Vec<UnitCoord> = Vec::with_capacity(coords.len());
                    for coord in coords {
                        n_coords.push(coord.into());
                    }
                    n_coords
                };
                let message: GossipMessage<B, H> = GossipMessage::FetchRequest(FetchRequest {
                    coords: n_coords,
                    peer_id: aux.child_creator(),
                });

                debug!(target: "afa", "Sending out message to our peers for epoch {}", self.epoch_id.0);
                let topic: <B as Block>::Hash = super::index_topic::<B>(aux.child_creator());

                self.network.lock().gossip_message(topic, message.encode(), false);

                self.sender.start_send(message).map_err(|_e| ())
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
}

pub(crate) struct NetworkBridge<B: Block, H, N: Network<B>> {
    network_service: N,
    gossip_engine: Arc<Mutex<GossipEngine<B>>>,
    gossip_validator: Arc<GossipValidator<B, H>>,
    peer_report_handle: Arc<Mutex<TracingUnboundedReceiver<PeerReport>>>,
}

// Need to figure out exactly how to pass Notifications to rush.

impl<B: Block, H: Hash, N: Network<B>> NetworkBridge<B, H, N> {
    pub(crate) fn new(network_service: N, config: Config, registry: Option<&Registry>) -> Self {
        let (gossip_validator, peer_report_handle) = {
            let (validator, peer_report_handle) = GossipValidator::<B, H>::new(registry);
            let validator = Arc::new(validator);
            let peer_report_handle = Arc::new(Mutex::new(peer_report_handle));
            (validator, peer_report_handle)
        };
        let gossip_engine = Arc::new(Mutex::new(GossipEngine::new(
            network_service.clone(),
            ALEPH_ENGINE_ID,
            ALEPH_PROTOCOL_NAME,
            gossip_validator.clone(),
        )));

        NetworkBridge {
            network_service,
            gossip_engine,
            gossip_validator,
            peer_report_handle,
        }
    }

    pub(crate) fn note_pending_fetch_request(&mut self, peer: PeerId, fetch_request: FetchRequest) {
        self.gossip_validator
            .note_pending_fetch_request(peer, fetch_request)
    }

    // TODO: keystore should be optional later.
    pub(crate) fn communication(
        &self,
        epoch: EpochId,
        keystore: AuthorityCryptoStore,
    ) -> (
        impl Stream<Item = NotificationIn<B::Hash, H>>,
        impl Sink<NotificationOut<B::Hash, H>> + Unpin,
    ) {
        let topic = epoch_topic::<B>(epoch);
        let mut gossip_engine = self.gossip_engine.clone();

        let incoming = gossip_engine
            .lock()
            .messages_for(topic)
            .filter_map(move |notification| {
                let decoded = GossipMessage::<B, H>::decode(&mut &notification.message[..]);
                let res = if let Ok(message) = decoded {
                    let notification = match message {
                        GossipMessage::Multicast(m) => {
                            let s_unit = m.signed_unit;
                            Some(NotificationIn::NewUnit(s_unit.unit))
                        }
                        // TODO: Wait for implementation of `ResponseParents` for `NotificationIn`.
                        // GossipMessage::FetchResponse(m) => {
                        //     let response = m.signed_unit;
                        // }
                        _ => None,
                    };
                    futures::future::ready(notification)
                } else {
                    trace!(target: "afa", "Skipping malformed incoming message {:?}", notification);
                    futures::future::ready(None)
                };
                res
            });

        // let outgoing =
    }
}
