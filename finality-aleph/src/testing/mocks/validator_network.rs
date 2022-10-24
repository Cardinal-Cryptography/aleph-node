use std::{
    collections::{BTreeMap, HashMap},
    io::Result as IoResult,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use aleph_primitives::{AuthorityId, KEY_TYPE};
use env_logger;
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use log::info;
use rand::{thread_rng, Rng};
use sc_service::{SpawnTaskHandle, TaskManager};
use sp_keystore::{testing::KeyStore, CryptoStore};
use tokio::{
    io::{duplex, AsyncRead, AsyncWrite, DuplexStream, ReadBuf},
    runtime::Handle,
    time::{interval, Duration},
};

use crate::{
    crypto::AuthorityPen,
    network::{mock::Channel, Data, Multiaddress, NetworkIdentity},
    validator_network::{
        mock::random_keys, service::Service, Dialer as DialerT, Listener as ListenerT, Network,
        Splittable,
    },
};

pub type MockMultiaddress = (AuthorityId, String);

impl Multiaddress for MockMultiaddress {
    type PeerId = AuthorityId;

    fn get_peer_id(&self) -> Option<Self::PeerId> {
        Some(self.0.clone())
    }

    fn add_matching_peer_id(self, peer_id: Self::PeerId) -> Option<Self> {
        match self.0 == peer_id {
            true => Some(self),
            false => None,
        }
    }
}

#[derive(Clone)]
pub struct MockNetwork<D: Data> {
    pub add_connection: Channel<(AuthorityId, Vec<MockMultiaddress>)>,
    pub remove_connection: Channel<AuthorityId>,
    pub send: Channel<(D, AuthorityId)>,
    pub next: Channel<D>,
    id: AuthorityId,
    addresses: Vec<MockMultiaddress>,
}

#[async_trait::async_trait]
impl<D: Data> Network<MockMultiaddress, D> for MockNetwork<D> {
    fn add_connection(&mut self, peer: AuthorityId, addresses: Vec<MockMultiaddress>) {
        self.add_connection.send((peer, addresses));
    }

    fn remove_connection(&mut self, peer: AuthorityId) {
        self.remove_connection.send(peer);
    }

    fn send(&self, data: D, recipient: AuthorityId) {
        self.send.send((data, recipient));
    }

    async fn next(&mut self) -> Option<D> {
        self.next.next().await
    }
}

impl<D: Data> NetworkIdentity for MockNetwork<D> {
    type PeerId = AuthorityId;
    type Multiaddress = MockMultiaddress;

    fn identity(&self) -> (Vec<Self::Multiaddress>, Self::PeerId) {
        (self.addresses.clone(), self.id.clone())
    }
}

impl<D: Data> MockNetwork<D> {
    pub async fn new(address: &str) -> Self {
        let key_store = Arc::new(KeyStore::new());
        let id: AuthorityId = key_store
            .ed25519_generate_new(KEY_TYPE, None)
            .await
            .unwrap()
            .into();
        let addresses = vec![(id.clone(), String::from(address))];
        MockNetwork {
            add_connection: Channel::new(),
            remove_connection: Channel::new(),
            send: Channel::new(),
            next: Channel::new(),
            addresses,
            id,
        }
    }

    // Consumes the network asserting there are no unreceived messages in the channels.
    pub async fn _close_channels(self) {
        assert!(self.add_connection.close().await.is_none());
        assert!(self.remove_connection.close().await.is_none());
        assert!(self.send.close().await.is_none());
        assert!(self.next.close().await.is_none());
    }
}

/// Bidirectional in-memory stream that closes abruptly after a specified
/// number of poll_write calls.
#[derive(Debug)]
pub struct UnreliableDuplexStream {
    stream: DuplexStream,
    counter: usize,
}

impl AsyncWrite for UnreliableDuplexStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        let mut _self = self.get_mut();
        if _self.counter == 0 {
            if Pin::new(&mut _self.stream).poll_shutdown(cx).is_pending() {
                return Poll::Pending;
            }
        } else {
            _self.counter -= 1;
        }
        Pin::new(&mut _self.stream).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

impl AsyncRead for UnreliableDuplexStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}

/// A stream that can be split into two instances of UnreliableDuplexStream.
#[derive(Debug)]
pub struct UnreliableSplittable {
    incoming_data: UnreliableDuplexStream,
    outgoing_data: UnreliableDuplexStream,
}

impl UnreliableSplittable {
    /// Create a pair of mock splittables connected to each other.
    pub fn new(max_buf_size: usize, ends_after: usize) -> (Self, Self) {
        let (in_a, out_b) = duplex(max_buf_size);
        let (in_b, out_a) = duplex(max_buf_size);
        (
            UnreliableSplittable {
                incoming_data: UnreliableDuplexStream {
                    stream: in_a,
                    counter: ends_after,
                },
                outgoing_data: UnreliableDuplexStream {
                    stream: out_a,
                    counter: ends_after,
                },
            },
            UnreliableSplittable {
                incoming_data: UnreliableDuplexStream {
                    stream: in_b,
                    counter: ends_after,
                },
                outgoing_data: UnreliableDuplexStream {
                    stream: out_b,
                    counter: ends_after,
                },
            },
        )
    }
}

impl AsyncRead for UnreliableSplittable {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().incoming_data).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnreliableSplittable {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.get_mut().outgoing_data).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().outgoing_data).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().outgoing_data).poll_shutdown(cx)
    }
}

impl Splittable for UnreliableSplittable {
    type Sender = UnreliableDuplexStream;
    type Receiver = UnreliableDuplexStream;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.outgoing_data, self.incoming_data)
    }
}

type Address = u32;
type Addresses = HashMap<AuthorityId, Vec<Address>>;
type Callers = HashMap<AuthorityId, (MockDialer, MockListener)>;
type Connection = UnreliableSplittable;

#[derive(Clone)]
pub struct MockDialer {
    c_connect: mpsc::UnboundedSender<(Address, oneshot::Sender<Connection>)>,
}

#[async_trait::async_trait]
impl DialerT<Address> for MockDialer {
    type Connection = Connection;
    type Error = std::io::Error;

    async fn connect(&mut self, addresses: Vec<Address>) -> Result<Self::Connection, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.c_connect
            .unbounded_send((addresses[0], tx))
            .expect("should send");
        Ok(rx.await.expect("should receive"))
    }
}

pub struct MockListener {
    c_new_connection: mpsc::UnboundedReceiver<Connection>,
}

#[async_trait::async_trait]
impl ListenerT for MockListener {
    type Connection = Connection;
    type Error = std::io::Error;

    async fn accept(&mut self) -> Result<Self::Connection, Self::Error> {
        Ok(self.c_new_connection.next().await.expect("should receive"))
    }
}

pub struct UnreliableConnectionMaker {
    c_dialers: mpsc::UnboundedReceiver<(Address, oneshot::Sender<Connection>)>,
    c_listeners: Vec<mpsc::UnboundedSender<Connection>>,
}

impl UnreliableConnectionMaker {
    pub fn new(ids: Vec<AuthorityId>) -> (Self, Callers, Addresses) {
        let mut c_listeners = Vec::with_capacity(ids.len());
        let mut callers = HashMap::with_capacity(ids.len());
        let (tx_dialer, c_dialers) = mpsc::unbounded();
        // create peer addresses that will be understood by the main loop in method run
        // each peer gets a one-element vector containing its index, so we'll be able
        // to retrieve proper communication channels
        let addr: Addresses = ids
            .clone()
            .into_iter()
            .zip(0..ids.len())
            .map(|(id, u)| (id, vec![u as u32]))
            .collect();
        // create callers for every peer, keep channels for communicating with them
        for id in ids.into_iter() {
            let (tx_listener, rx_listener) = mpsc::unbounded();
            let dialer = MockDialer {
                c_connect: tx_dialer.clone(),
            };
            let listener = MockListener {
                c_new_connection: rx_listener,
            };
            c_listeners.push(tx_listener);
            callers.insert(id, (dialer, listener));
        }
        (
            UnreliableConnectionMaker {
                c_dialers,
                c_listeners,
            },
            callers,
            addr,
        )
    }

    pub async fn run(&mut self, connections_end_after: usize) {
        loop {
            info!(target: "validator-network", "UnreliableConnectionMaker: waiting for new request...");
            let (addr, c) = self.c_dialers.next().await.expect("should receive");
            info!(target: "validator-network", "UnreliableConnectionMaker: received request");
            let (l_stream, r_stream) = Connection::new(4096, connections_end_after);
            info!(target: "validator-network", "UnreliableConnectionMaker: sending stream");
            c.send(l_stream).expect("should send");
            self.c_listeners[addr as usize]
                .unbounded_send(r_stream)
                .expect("should send");
        }
    }
}

type MockData = u32;

fn spawn_peer(
    pen: AuthorityPen,
    addr: Addresses,
    n_msg: usize,
    dialer: MockDialer,
    listener: MockListener,
    report: mpsc::UnboundedSender<(AuthorityId, usize)>,
    spawn_handle: SpawnTaskHandle,
) {
    let our_id = pen.authority_id();
    let (service, mut interface) = Service::new(dialer, listener, pen, spawn_handle);
    // run the service
    tokio::spawn(async {
        let (_exit, rx) = oneshot::channel();
        service.run(rx).await;
    });
    // start connecting with the peers
    let mut peer_ids = Vec::with_capacity(addr.len());
    for (id, addrs) in addr.into_iter() {
        interface.add_connection(id.clone(), addrs);
        peer_ids.push(id);
    }
    // peer main loop
    // we send random messages to random peers
    // a message is a number in range 0..n_msg
    // we also keep a list of messages received at least once
    // on receiving a message we report the total number of distinct messages received so far
    // the goal is to receive every message at least once
    tokio::spawn(async move {
        let mut received: Vec<bool> = vec![false; n_msg];
        let mut send_ticker = tokio::time::interval(Duration::from_millis(5));
        loop {
            tokio::select! {
                _ = send_ticker.tick() => {
                    // generate random message
                    let data: MockData = thread_rng().gen_range(0..n_msg) as u32;
                    // choose a peer
                    let peer: AuthorityId = peer_ids[thread_rng().gen_range(0..peer_ids.len())].clone();
                    // send
                    interface.send(data, peer);
                },
                data = interface.next() => {
                    // receive the message
                    let data: MockData = data.expect("next should not be closed");
                    // mark the message as received, we do not care about sender's identity
                    received[data as usize] = true;
                    // report the number of received messages
                    report.unbounded_send((our_id.clone(), received.iter().filter(|x| **x).count())).expect("should send");
                },
            };
        }
    });
}

#[tokio::test]
async fn integration() {
    env_logger::init();
    const N_PEERS: usize = 5;
    const N_MSG: usize = 400;
    const CONNECTIONS_END_AFTER: usize = 40000;
    const STATUS_REPORT_INTERVAL: Duration = Duration::from_secs(3);
    // create spawn_handle, we need to keep the task_manager
    let task_manager =
        TaskManager::new(Handle::current(), None).expect("should create TaskManager");
    let spawn_handle = task_manager.spawn_handle();
    // create peer identities
    let keys = random_keys(N_PEERS).await;
    // prepare and run the manager
    let (mut connection_manager, mut callers, addr) =
        UnreliableConnectionMaker::new(keys.keys().cloned().collect());
    tokio::spawn(async move {
        connection_manager.run(CONNECTIONS_END_AFTER).await;
    });
    // channel for receiving status updates from spawned peers
    let (tx_report, mut rx_report) = mpsc::unbounded::<(AuthorityId, usize)>();
    let mut reports: BTreeMap<AuthorityId, usize> =
        keys.keys().cloned().map(|id| (id, 0)).collect();
    // spawn peers
    for (id, pen) in keys.into_iter() {
        let mut addr = addr.clone();
        addr.remove(&pen.authority_id());
        let (dialer, listener) = callers.remove(&id).expect("should contain all ids");
        spawn_peer(
            pen,
            addr,
            N_MSG,
            dialer,
            listener,
            tx_report.clone(),
            spawn_handle.clone(),
        );
    }
    let mut status_ticker = interval(STATUS_REPORT_INTERVAL);
    loop {
        tokio::select! {
            // got new incoming connection from the listener - spawn an incoming worker
            maybe_report = rx_report.next() => match maybe_report {
                Some((peer_id, n_msg)) => {
                    reports.insert(peer_id, n_msg);
                    if reports.values().all(|&x| x == N_MSG) { return; }
                },
                None => panic!("should receive"),
            },
            _ = status_ticker.tick() => {
                info!(target: "validator-network", "Peers received {:?} out of {}", reports.values(), N_MSG);
            }
        };
    }
}
