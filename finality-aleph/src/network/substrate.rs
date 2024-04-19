use std::{collections::HashMap, fmt, iter, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::stream::{Fuse, Stream, StreamExt};
use log::{error, trace, warn};
use parking_lot::Mutex;
use sc_network::{
    multiaddr::Protocol as MultiaddressProtocol, Event as LegacySubstrateEvent, Multiaddr,
    NetworkEventStream as _, NetworkPeers, NetworkService,
    PeerId, ProtocolName,
    service::traits::{NotificationEvent as SubstrateEvent, ValidationResult},
};
use sc_network_common::ExHashT;
use sc_network_sync::{SyncEvent, SyncEventStream, SyncingService};
use sp_runtime::traits::Block;

use crate::network::gossip::{Event, EventStream, NetworkSender, Protocol, RawNetwork};

/// Name of the network protocol used by Aleph Zero to disseminate validator
/// authentications.
const AUTHENTICATION_PROTOCOL_NAME: &str = "/auth/0";

/// Name of the network protocol used by Aleph Zero to synchronize the block state.
const BLOCK_SYNC_PROTOCOL_NAME: &str = "/sync/0";

/// Convert protocols to their names and vice versa.
#[derive(Clone)]
pub struct ProtocolNaming {
    authentication_name: ProtocolName,
    block_sync_name: ProtocolName,
    protocols_by_name: HashMap<ProtocolName, Protocol>,
}

impl ProtocolNaming {
    /// Create a new protocol naming scheme with the given chain prefix.
    pub fn new(chain_prefix: String) -> Self {
        let authentication_name: ProtocolName =
            format!("{chain_prefix}{AUTHENTICATION_PROTOCOL_NAME}").into();
        let mut protocols_by_name = HashMap::new();
        protocols_by_name.insert(authentication_name.clone(), Protocol::Authentication);
        let block_sync_name: ProtocolName =
            format!("{chain_prefix}{BLOCK_SYNC_PROTOCOL_NAME}").into();
        protocols_by_name.insert(block_sync_name.clone(), Protocol::BlockSync);
        ProtocolNaming {
            authentication_name,
            block_sync_name,
            protocols_by_name,
        }
    }

    /// Returns the canonical name of the protocol.
    pub fn protocol_name(&self, protocol: &Protocol) -> ProtocolName {
        use Protocol::*;
        match protocol {
            Authentication => self.authentication_name.clone(),
            BlockSync => self.block_sync_name.clone(),
        }
    }

    /// Returns the fallback names of the protocol.
    pub fn fallback_protocol_names(&self, _protocol: &Protocol) -> Vec<ProtocolName> {
        Vec::new()
    }

    /// Attempts to convert the protocol name to a protocol.
    fn to_protocol(&self, protocol_name: &str) -> Option<Protocol> {
        self.protocols_by_name.get(protocol_name).copied()
    }
}

#[derive(Debug)]
pub enum SenderError {
    CannotCreateSender(PeerId, Protocol),
    LostConnectionToPeer(PeerId),
    LostConnectionToPeerReady(PeerId),
}

impl fmt::Display for SenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SenderError::CannotCreateSender(peer_id, protocol) => {
                write!(
                    f,
                    "Can not create sender to peer {peer_id:?} with protocol {protocol:?}"
                )
            }
            SenderError::LostConnectionToPeer(peer_id) => {
                write!(
                    f,
                    "Lost connection to peer {peer_id:?} while preparing sender"
                )
            }
            SenderError::LostConnectionToPeerReady(peer_id) => {
                write!(
                    f,
                    "Lost connection to peer {peer_id:?} after sender was ready"
                )
            }
        }
    }
}

impl std::error::Error for SenderError {}

pub struct SubstrateNetworkSender {
    message_sink: Box<dyn sc_network::service::traits::MessageSink>,
    peer_id: PeerId,
}

#[async_trait]
impl NetworkSender for SubstrateNetworkSender {
    type SenderError = SenderError;

    async fn send<'a>(
        &'a self,
        data: impl Into<Vec<u8>> + Send + Sync + 'static,
    ) -> Result<(), SenderError> {
        self.message_sink.send_async_notification(
            data.into()
        ).await.map_err(|_| SenderError::LostConnectionToPeer(self.peer_id))
    }
}

pub struct NetworkEventStream<B: Block, H: ExHashT> {
    stream: Fuse<Pin<Box<dyn Stream<Item = LegacySubstrateEvent> + Send>>>,
    sync_stream: Fuse<Pin<Box<dyn Stream<Item = SyncEvent> + Send>>>,
    sync_notification_service: Box<dyn sc_network::config::NotificationService>,
    authentication_notification_service: Box<dyn sc_network::config::NotificationService>,
    naming: ProtocolNaming,
    network: Arc<NetworkService<B, H>>,
}

#[async_trait]
impl<B: Block, H: ExHashT> EventStream<PeerId> for NetworkEventStream<B, H> {
    async fn next_event(&mut self) -> Option<Event<PeerId>> {
        use Event::*;
        use SyncEvent::*;
        loop {
            tokio::select! {
                Some(event) = self.sync_notification_service.next_event() => {
                    use SubstrateEvent::*;
                    match event {
                        ValidateInboundSubstream {
                            peer: _,
                            handshake: _,
                            result_tx,
                        } => {
                            let _ = result_tx.send(ValidationResult::Accept);
                            continue
                        },
                        NotificationStreamOpened {
                            peer,
                            ..
                        } => return Some(StreamOpened(peer, Protocol::BlockSync)),
                        NotificationStreamClosed {
                            peer,
                        } => return Some(StreamClosed(peer, Protocol::BlockSync)),
                        NotificationReceived {
                            peer,
                            notification,
                        } => return Some(Messages(
                            peer,
                            vec![(Protocol::BlockSync, notification.into())],
                        )),
                    }
                },

                Some(event) = self.authentication_notification_service.next_event() => {
                    use SubstrateEvent::*;
                    match event {
                        ValidateInboundSubstream {
                            peer: _,
                            handshake: _,
                            result_tx,
                        } => {
                            let _ = result_tx.send(ValidationResult::Accept);
                            continue
                        },
                        NotificationStreamOpened {
                            peer,
                            ..
                        } => return Some(StreamOpened(peer, Protocol::Authentication)),
                        NotificationStreamClosed {
                            peer,
                        } => return Some(StreamClosed(peer, Protocol::Authentication)),
                        NotificationReceived {
                            peer,
                            notification,
                        } => return Some(Messages(
                            peer,
                            vec![(Protocol::Authentication, notification.into())],
                        )),
                    }
                },

                Some(event) = self.stream.next() => {
                    use LegacySubstrateEvent::*;
                    match event {
                        NotificationStreamOpened {
                            remote, protocol, ..
                        } => match self.naming.to_protocol(protocol.as_ref()) {
                            Some(protocol) => return Some(StreamOpened(remote, protocol)),
                            None => continue,
                        },
                        NotificationStreamClosed { remote, protocol } => {
                            match self.naming.to_protocol(protocol.as_ref()) {
                                Some(protocol) => return Some(StreamClosed(remote, protocol)),
                                None => continue,
                            }
                        }
                        NotificationsReceived { messages, remote } => {
                            return Some(Messages(
                                remote,
                                messages
                                    .into_iter()
                                    .filter_map(|(protocol, data)| {
                                        self.naming
                                            .to_protocol(protocol.as_ref())
                                            .map(|protocol| (protocol, data))
                                    })
                                    .collect(),
                            ));
                        }
                        Dht(_) => continue,
                    }
                },

                Some(event) = self.sync_stream.next() => {
                    match event {
                        PeerConnected(remote) => {
                            let multiaddress: Multiaddr =
                                iter::once(MultiaddressProtocol::P2p(remote.into())).collect();
                            trace!(target: "aleph-network", "Connected event from address {:?}", multiaddress);
                            if let Err(e) = self.network.add_peers_to_reserved_set(
                                self.naming.protocol_name(&Protocol::Authentication),
                                iter::once(multiaddress.clone()).collect(),
                            ) {
                                error!(target: "aleph-network", "add_reserved failed for authentications: {}", e);
                            }
                            if let Err(e) = self.network.add_peers_to_reserved_set(
                                self.naming.protocol_name(&Protocol::BlockSync),
                                iter::once(multiaddress).collect(),
                            ) {
                                error!(target: "aleph-network", "add_reserved failed for block sync: {}", e);
                            }
                            continue;
                        }
                        PeerDisconnected(remote) => {
                            trace!(target: "aleph-network", "Disconnected event for peer {:?}", remote);
                            let addresses: Vec<_> = iter::once(remote).collect();
                            if let Err(e) = self.network.remove_peers_from_reserved_set(
                                self.naming.protocol_name(&Protocol::Authentication),
                                addresses.clone(),
                            ) {
                                warn!(target: "aleph-network", "Error while removing peer from Protocol::Authentication reserved set: {}", e)
                            }
                            if let Err(e) = self.network.remove_peers_from_reserved_set(
                                self.naming.protocol_name(&Protocol::BlockSync),
                                addresses,
                            ) {
                                warn!(target: "aleph-network", "Error while removing peer from Protocol::BlockSync reserved set: {}", e)
                            }
                            continue;
                        }
                    }
                },

                else => return None,
            }
        }
    }
}

/// A wrapper around the substrate network that includes information about protocol names.
pub struct SubstrateNetwork<B: Block, H: ExHashT> {
    network: Arc<NetworkService<B, H>>,
    sync_network: Arc<SyncingService<B>>,
    naming: ProtocolNaming,
    sync_notification_service: Mutex<Box<dyn sc_network::config::NotificationService>>,
    authentication_notification_service: Mutex<Box<dyn sc_network::config::NotificationService>>,
}

impl<B: Block, H: ExHashT> Clone for SubstrateNetwork<B, H> {
    fn clone(&self) -> Self {
        Self {
            network: self.network.clone(),
            sync_network: self.sync_network.clone(),
            naming: self.naming.clone(),
            sync_notification_service: Mutex::new(
                self.sync_notification_service
                    .lock()
                    .clone()
                    .expect("should clone"),
            ),
            authentication_notification_service: Mutex::new(
                self.authentication_notification_service
                    .lock()
                    .clone()
                    .expect("should clone"),
            ),
        }
    }
}

impl<B: Block, H: ExHashT> SubstrateNetwork<B, H> {
    /// Create a new substrate network wrapper.
    pub fn new(
        network: Arc<NetworkService<B, H>>,
        sync_network: Arc<SyncingService<B>>,
        naming: ProtocolNaming,
        sync_notification_service: Box<dyn sc_network::config::NotificationService>,
        authentication_notification_service: Box<dyn sc_network::config::NotificationService>,
    ) -> Self {
        SubstrateNetwork {
            network,
            sync_network,
            naming,
            sync_notification_service: Mutex::new(sync_notification_service),
            authentication_notification_service: Mutex::new(authentication_notification_service),
        }
    }

    pub fn event_stream(&self) -> NetworkEventStream<B, H> {
        NetworkEventStream {
            stream: self.network.event_stream("aleph-network").fuse(),
            sync_stream: self
                .sync_network
                .event_stream("aleph-syncing-network")
                .fuse(),
            sync_notification_service: self.sync_notification_service.lock().clone().expect("should clone"),
            authentication_notification_service: self.authentication_notification_service.lock().clone().expect("should clone"),
            naming: self.naming.clone(),
            network: self.network.clone(),
        }
    }
}

impl<B: Block, H: ExHashT> RawNetwork for SubstrateNetwork<B, H> {
    type SenderError = SenderError;
    type NetworkSender = SubstrateNetworkSender;
    type PeerId = PeerId;

    fn sender(
        &self,
        peer_id: Self::PeerId,
        protocol: Protocol,
    ) -> Result<Self::NetworkSender, Self::SenderError> {
        Ok(SubstrateNetworkSender {
            message_sink: match protocol {
                Protocol::Authentication => &self.authentication_notification_service,
                Protocol::BlockSync => &self.sync_notification_service,
            }.lock().message_sink(&peer_id).expect("should return a sink"),
            peer_id,
        })
    }
}
