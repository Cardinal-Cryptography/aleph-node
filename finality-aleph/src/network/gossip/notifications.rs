use core::fmt;
use std::{
    collections::HashSet,
    fmt::{Debug, Formatter},
};

use log::{debug, info, trace, warn};
use rand::{seq::IteratorRandom, thread_rng};
use sc_network::service::traits::{NotificationEvent as SubstrateEvent, ValidationResult};
pub use sc_network::PeerId;
use tokio::time;

use crate::{
    network::{
        gossip::{Network, Protocol},
        Data,
    },
    STATUS_REPORT_INTERVAL,
};

const LOG_TARGET: &str = "aleph-network";

pub struct Service {
    _protocol: Protocol,
    connected_peers: HashSet<PeerId>,
    service: Box<dyn sc_network::config::NotificationService>,
}

#[derive(Debug)]
pub enum GossipServiceError {
    NetworkStreamTerminated,
}

impl fmt::Display for GossipServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GossipServiceError::NetworkStreamTerminated => write!(f, "Network event stream ended."),
        }
    }
}

impl Service {
    pub fn new(
        protocol: Protocol,
        service: Box<dyn sc_network::config::NotificationService>,
    ) -> Self {
        Service {
            _protocol: protocol,
            connected_peers: HashSet::new(),
            service,
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
                return None;
            }
            NotificationStreamOpened { peer, .. } => {
                self.connected_peers.insert(peer);
                return None;
            }
            NotificationStreamClosed { peer } => {
                self.connected_peers.remove(&peer);
                return None;
            }
            NotificationReceived { peer, notification } => return Some((notification, peer)),
        };
    }

    fn status_report(&self) {
        let mut status = String::from("Network status report: ");

        status.push_str(&format!(
            "connected peers - {:?}; ",
            self.connected_peers.len()
        ));

        info!(target: LOG_TARGET, "{}", status);
    }
}

#[async_trait::async_trait]
impl<D: Data> Network<D> for Service {
    type Error = GossipServiceError;
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
            Some(peer_id) => peer_id.clone(),
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
        let mut status_ticker = time::interval(STATUS_REPORT_INTERVAL);
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
                },
            }
        }
    }
}
