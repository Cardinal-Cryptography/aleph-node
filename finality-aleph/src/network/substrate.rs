use core::fmt;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter},
    hash::Hash,
    iter,
    pin::Pin,
    sync::Arc,
};

use futures::stream::{Fuse, Stream, StreamExt};
use log::{debug, error, info, trace, warn};
use rand::{seq::IteratorRandom, thread_rng};
pub use sc_network::PeerId;
use sc_network::{
    multiaddr::Protocol as MultiaddressProtocol,
    service::traits::{NotificationEvent as SubstrateEvent, ValidationResult},
    Multiaddr, NetworkPeers, NetworkService, ProtocolName,
};
use sc_network_common::ExHashT;
use sc_network_sync::{SyncEvent, SyncEventStream, SyncingService};
use sp_runtime::traits::Block;
use tokio::time;

use crate::{
    network::{Data, GossipNetwork},
    STATUS_REPORT_INTERVAL,
};

const LOG_TARGET: &str = "aleph-network";

/// Name of the network protocol used by Aleph Zero to disseminate validator
/// authentications.
const AUTHENTICATION_PROTOCOL_NAME: &str = "/auth/0";

/// Name of the network protocol used by Aleph Zero to synchronize the block state.
const BLOCK_SYNC_PROTOCOL_NAME: &str = "/sync/0";

/// Protocols used by the network.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum Protocol {
    /// The authentication protocol is used for validator discovery.
    Authentication,
    /// The block synchronization protocol.
    BlockSync,
}

/// Convert protocols to their names and vice versa.
#[derive(Clone)]
pub struct ProtocolNaming {
    authentication_name: ProtocolName,
    block_sync_name: ProtocolName,
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
}

#[derive(Debug)]
pub enum SyncNetworkServiceError {
    NetworkStreamTerminated,
}

impl fmt::Display for SyncNetworkServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkStreamTerminated => write!(f, "Network event stream ended."),
        }
    }
}

/// Service responsible for handling network events emitted by the base sync protocol.
pub struct SyncNetworkService<B: Block, H: ExHashT> {
    sync_stream: Fuse<Pin<Box<dyn Stream<Item = SyncEvent> + Send>>>,
    naming: ProtocolNaming,
    network: Arc<NetworkService<B, H>>,
}

impl<B: Block, H: ExHashT> SyncNetworkService<B, H> {
    pub fn new(
        network: Arc<NetworkService<B, H>>,
        sync_network: Arc<SyncingService<B>>,
        naming: ProtocolNaming,
    ) -> Self {
        Self {
            sync_stream: sync_network.event_stream("aleph-syncing-network").fuse(),
            naming,
            network,
        }
    }

    fn protocols(&self) -> [Protocol; 2] {
        [Protocol::Authentication, Protocol::BlockSync]
    }

    fn peer_connected(&mut self, remote: PeerId) {
        let multiaddress: Multiaddr =
            iter::once(MultiaddressProtocol::P2p(remote.into())).collect();
        trace!(target: LOG_TARGET, "Connected event from address {:?}", multiaddress);

        for protocol in self.protocols() {
            let name = self.naming.protocol_name(&protocol);
            if let Err(e) = self
                .network
                .add_peers_to_reserved_set(name.clone(), iter::once(multiaddress.clone()).collect())
            {
                error!(target: LOG_TARGET, "add_reserved failed for {}: {}", name, e);
            }
        }
    }

    fn peer_disconnected(&mut self, remote: PeerId) {
        trace!(target: LOG_TARGET, "Disconnected event for peer {:?}", remote);
        let addresses: Vec<_> = iter::once(remote).collect();

        for protocol in self.protocols() {
            let name = self.naming.protocol_name(&protocol);
            if let Err(e) = self
                .network
                .remove_peers_from_reserved_set(name.clone(), addresses.clone())
            {
                warn!(target: LOG_TARGET, "Error while removing peer from {} reserved set: {}", name, e)
            }
        }
    }

    pub async fn run(mut self) -> Result<(), SyncNetworkServiceError> {
        use SyncEvent::*;
        loop {
            match self.sync_stream.next().await {
                Some(event) => match event {
                    PeerConnected(remote) => self.peer_connected(remote),
                    PeerDisconnected(remote) => self.peer_disconnected(remote),
                },
                None => return Err(SyncNetworkServiceError::NetworkStreamTerminated),
            }
        }
    }
}

/// A thin wrapper around sc_network::config::NotificationService that stores a list
/// of all currently connected peers, and introduces a few convenience methods to
/// allow broadcasting messages and sending data to random peers.
pub struct ProtocolNetwork {
    service: Box<dyn sc_network::config::NotificationService>,
    connected_peers: HashSet<PeerId>,
    last_status_report: time::Instant,
}

impl ProtocolNetwork {
    pub fn new(service: Box<dyn sc_network::config::NotificationService>) -> Self {
        Self {
            service,
            connected_peers: HashSet::new(),
            last_status_report: time::Instant::now(),
        }
    }

    fn random_peer<'a>(&'a self, peer_ids: &'a HashSet<PeerId>) -> Option<&'a PeerId> {
        peer_ids
            .intersection(&self.connected_peers)
            .choose(&mut thread_rng())
            .or_else(|| self.connected_peers.iter().choose(&mut thread_rng()))
    }

    fn handle_network_event(&mut self, event: SubstrateEvent) -> Option<(Vec<u8>, PeerId)> {
        use SubstrateEvent::*;
        match event {
            ValidateInboundSubstream {
                peer: _,
                handshake: _,
                result_tx,
            } => {
                let _ = result_tx.send(ValidationResult::Accept);
                None
            }
            NotificationStreamOpened { peer, .. } => {
                self.connected_peers.insert(peer);
                None
            }
            NotificationStreamClosed { peer } => {
                self.connected_peers.remove(&peer);
                None
            }
            NotificationReceived { peer, notification } => Some((notification, peer)),
        }
    }

    fn status_report(&self) {
        let mut status = String::from("Network status report: ");
        status.push_str(&format!(
            "{} connected peers - {:?}; ",
            self.service.protocol(),
            self.connected_peers.len()
        ));
        info!(target: LOG_TARGET, "{}", status);
    }
}

#[derive(Debug)]
pub enum ProtocolNetworkError {
    NetworkStreamTerminated,
}

impl fmt::Display for ProtocolNetworkError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolNetworkError::NetworkStreamTerminated => {
                write!(f, "Notifications event stream ended.")
            }
        }
    }
}

#[async_trait::async_trait]
impl<D: Data> GossipNetwork<D> for ProtocolNetwork {
    type Error = ProtocolNetworkError;
    type PeerId = PeerId;

    fn send_to(&mut self, data: D, peer_id: PeerId) -> Result<(), Self::Error> {
        trace!(
            target: LOG_TARGET,
            "Sending block sync data to peer {:?}.",
            peer_id,
        );
        self.service.send_sync_notification(&peer_id, data.encode());
        Ok(())
    }

    fn send_to_random(&mut self, data: D, peer_ids: HashSet<PeerId>) -> Result<(), Self::Error> {
        trace!(
            target: LOG_TARGET,
            "Sending data to random peer among {:?}.",
            peer_ids,
        );
        let peer_id = match self.random_peer(&peer_ids) {
            Some(peer_id) => *peer_id,
            None => {
                debug!(
                    target: LOG_TARGET,
                    "Failed to send message to random peer, no peers are available."
                );
                return Ok(());
            }
        };
        self.send_to(data, peer_id)
    }

    fn broadcast(&mut self, data: D) -> Result<(), Self::Error> {
        for peer in self.connected_peers.clone() {
            self.send_to(data.clone(), peer)?;
        }
        Ok(())
    }

    async fn next(&mut self) -> Result<(D, PeerId), Self::Error> {
        let mut status_ticker = time::interval_at(
            self.last_status_report
                .checked_add(STATUS_REPORT_INTERVAL)
                .unwrap_or(time::Instant::now()),
            STATUS_REPORT_INTERVAL,
        );
        loop {
            tokio::select! {
                maybe_event = self.service.next_event() => {
                    let event = maybe_event.ok_or(Self::Error::NetworkStreamTerminated)?;
                    if let Some((message, peer_id)) = self.handle_network_event(event) {
                        match D::decode(&mut &message[..]) {
                            Ok(message) => return Ok((message, peer_id)),
                            Err(e) => {
                                warn!(
                                    target: LOG_TARGET,
                                    "Error decoding message: {}", e
                                )
                            },
                        }
                    }
                },
                _ = status_ticker.tick() => {
                    self.status_report();
                    self.last_status_report = time::Instant::now();
                },
            }
        }
    }
}
