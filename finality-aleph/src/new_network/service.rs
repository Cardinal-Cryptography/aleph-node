use crate::new_network::{
    ConnectionCommand, Data, DataCommand, Network, PeerId, Protocol, ALEPH_PROTOCOL_NAME,
    ALEPH_VALIDATOR_PROTOCOL_NAME,
};
use futures::{channel::mpsc, StreamExt};
use log::{debug, error, trace};
use sc_network::{multiaddr, Event};
use std::{
    borrow::Cow,
    collections::{HashSet, VecDeque},
    iter,
};

struct Service<N: Network, D: Data> {
    network: N,
    messages_from_user: mpsc::UnboundedReceiver<(D, DataCommand)>,
    messages_for_user: mpsc::UnboundedSender<D>,
    commands_from_manager: mpsc::UnboundedReceiver<ConnectionCommand>,
    connected_peers: HashSet<PeerId>,
    to_send: VecDeque<(D, PeerId, Protocol)>,
}

pub struct IO<D: Data> {
    messages_from_user: mpsc::UnboundedReceiver<(D, DataCommand)>,
    messages_for_user: mpsc::UnboundedSender<D>,
    commands_from_manager: mpsc::UnboundedReceiver<ConnectionCommand>,
}

impl<N: Network, D: Data> Service<N, D> {
    pub fn new(network: N, io: IO<D>) -> Service<N, D> {
        let IO {
            messages_from_user,
            messages_for_user,
            commands_from_manager,
        } = io;
        Service {
            network,
            messages_from_user,
            messages_for_user,
            commands_from_manager,
            connected_peers: HashSet::new(),
            to_send: VecDeque::new(),
        }
    }

    fn send_to_peer(&mut self, data: D, peer: PeerId, protocol: Protocol) {
        self.to_send.push_back((data, peer, protocol));
    }

    fn broadcast(&mut self, data: D) {
        for peer in self.connected_peers.clone() {
            // We only broadcast authentication information in this sense, so we use the generic
            // Protocol.
            self.send_to_peer(data.clone(), peer, Protocol::Generic);
        }
    }

    fn handle_network_event(&mut self, event: Event) -> Result<(), mpsc::TrySendError<D>> {
        match event {
            Event::SyncConnected { remote } => {
                trace!(target: "aleph-network", "SyncConnected event for peer {:?}", remote);
                let addr = iter::once(multiaddr::Protocol::P2p(remote.into())).collect();
                self.network.add_reserved(
                    iter::once(addr).collect(),
                    Cow::Borrowed(ALEPH_PROTOCOL_NAME),
                );
            }
            Event::SyncDisconnected { remote } => {
                trace!(target: "aleph-network", "SyncDisconnected event for peer {:?}", remote);
                self.network.remove_reserved(
                    iter::once(remote.into()).collect(),
                    Cow::Borrowed(ALEPH_PROTOCOL_NAME),
                );
            }
            Event::NotificationStreamOpened {
                remote, protocol, ..
            } => {
                if protocol == ALEPH_PROTOCOL_NAME {
                    self.connected_peers.insert(remote.into());
                }
            }
            Event::NotificationStreamClosed { remote, protocol } => {
                if protocol == ALEPH_PROTOCOL_NAME {
                    self.connected_peers.remove(&remote.into());
                }
            }
            Event::NotificationsReceived {
                remote: _,
                messages,
            } => {
                for (protocol, data) in messages.into_iter() {
                    if protocol == ALEPH_PROTOCOL_NAME || protocol == ALEPH_VALIDATOR_PROTOCOL_NAME
                    {
                        match D::decode(&mut &data[..]) {
                            Ok(message) => self.messages_for_user.unbounded_send(message)?,
                            Err(e) => {
                                debug!(target: "aleph-network", "Error decoding message: {}", e)
                            }
                        }
                    }
                }
            }
            // Irrelevant for us, ignore.
            Event::Dht(_) => {}
        }
        Ok(())
    }

    fn on_manager_command(&self, command: ConnectionCommand) {
        use ConnectionCommand::*;
        match command {
            AddReserved(addresses) => self
                .network
                .add_reserved(addresses, Cow::Borrowed(ALEPH_VALIDATOR_PROTOCOL_NAME)),
            DelReserved(peers) => self
                .network
                .remove_reserved(peers, Cow::Borrowed(ALEPH_VALIDATOR_PROTOCOL_NAME)),
        }
    }

    fn on_user_command(&mut self, data: D, command: DataCommand) {
        use DataCommand::*;
        match command {
            Broadcast => self.broadcast(data),
            SendTo(peer, protocol) => self.send_to_peer(data, peer, protocol),
        }
    }

    async fn send(network: &N, send_queue: &mut VecDeque<(D, PeerId, Protocol)>) -> Option<()> {
        loop {
            if let Some((data, peer, protocol)) = send_queue.front() {
                if let Ok(()) = network.send(data.encode(), *peer, protocol.name()).await {
                    send_queue.pop_front();
                    return Some(());
                } else {
                    debug!(target: "aleph-network", "Failed sending data to peer {:?}", peer);
                }
                send_queue.pop_front();
            } else {
                trace!(target: "aleph-network", "Attempted to send data with empty send queue.");
                return None;
            }
        }
    }

    pub async fn run(mut self) {
        let mut events_from_network = self.network.event_stream();
        loop {
            tokio::select! {
                maybe_event = events_from_network.next() => match maybe_event {
                    Some(event) => if let Err(e) = self.handle_network_event(event) {
                        error!(target: "aleph-network", "Cannot forward messages to user: {:?}", e);
                        return;
                    },
                    None => {
                        error!(target: "aleph-network", "Network event stream ended.");
                        return;
                    }
                },
                maybe_command = self.commands_from_manager.next() => match maybe_command {
                    Some(command) => self.on_manager_command(command),
                    None => {
                        error!(target: "aleph-network", "Manager command stream ended.");
                        return;
                    }
                },
                maybe_message = self.messages_from_user.next() => match maybe_message {
                    Some((data, command)) => self.on_user_command(data, command),
                    None => {
                        error!(target: "aleph-network", "User message stream ended.");
                        return;
                    }
                },
                Some(()) =  Self::send(&self.network, &mut self.to_send) => { },
            }
        }
    }
}
