use std::sync::Arc;
#[cfg(test)]
use std::{
    io::Result as IoResult,
    pin::Pin,
    task::{Context, Poll},
};

use aleph_primitives::{AuthorityId, KEY_TYPE};
use log::info;
use sp_keystore::{testing::KeyStore, CryptoStore};
use tokio::io::{duplex, AsyncRead, AsyncWrite, DuplexStream, ReadBuf};

use crate::{crypto::AuthorityPen, validator_network::Splittable};

/// Create a single authority id and pen of the same type, not related to each other.
pub async fn keys() -> (AuthorityId, AuthorityPen) {
    let keystore = Arc::new(KeyStore::new());
    let id: AuthorityId = keystore
        .ed25519_generate_new(KEY_TYPE, None)
        .await
        .unwrap()
        .into();
    let pen = AuthorityPen::new(id.clone(), keystore)
        .await
        .expect("keys shoud sign successfully");
    (id, pen)
}

#[derive(Debug)]
pub struct FlakyDuplexStream(DuplexStream, u32);

impl AsyncWrite for FlakyDuplexStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        let _self = self.get_mut();
        _self.1 += 1;
        if _self.1 >= 30 {
            println!("Terminating flaky stream");
            match Pin::new(&mut _self.0).poll_shutdown(cx) {
                Poll::Pending => {
                    return Poll::Pending;
                }
                _ => (),
            }
        }
        Pin::new(&mut _self.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}

impl AsyncRead for FlakyDuplexStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

/// A mock that can be split into two streams.
#[derive(Debug)]
pub struct MockSplittable {
    incoming_data: FlakyDuplexStream,
    outgoing_data: FlakyDuplexStream,
}

impl MockSplittable {
    /// Create a pair of mock splittables connected to each other.
    pub fn new(max_buf_size: usize) -> (Self, Self) {
        let (in_a, out_b) = duplex(max_buf_size);
        let (in_b, out_a) = duplex(max_buf_size);
        (
            MockSplittable {
                // incoming_data: in_a,
                // outgoing_data: out_a,
                incoming_data: FlakyDuplexStream(in_a, 0),
                outgoing_data: FlakyDuplexStream(out_a, 0),
            },
            MockSplittable {
                // incoming_data: in_b,
                // outgoing_data: out_b,
                incoming_data: FlakyDuplexStream(in_b, 0),
                outgoing_data: FlakyDuplexStream(out_b, 0),
            },
        )
    }
}

impl AsyncRead for MockSplittable {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().incoming_data).poll_read(cx, buf)
    }
}

impl AsyncWrite for MockSplittable {
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

impl Splittable for MockSplittable {
    type Sender = FlakyDuplexStream;
    type Receiver = FlakyDuplexStream;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.outgoing_data, self.incoming_data)
    }
}

use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};

use crate::validator_network::{Dialer as DialerT, Listener as ListenerT};

type Multiaddress = u32;
type Connection = MockSplittable;

pub struct ConnectionMaker {
    c_dialers: mpsc::UnboundedReceiver<(Multiaddress, oneshot::Sender<Connection>)>,
    c_listeners: Vec<mpsc::UnboundedSender<Connection>>,
}

impl ConnectionMaker {
    pub fn new(n_peers: usize) -> (Self, Vec<(Dialer, Listener)>) {
        let mut c_listeners = vec![];
        let mut out = vec![];
        let (tx_dialer, c_dialers) = mpsc::unbounded();

        for _ in 0..n_peers {
            let (tx_listener, rx_listener) = mpsc::unbounded();
            let dialer = Dialer {
                c_connect: tx_dialer.clone(),
            };
            let listener = Listener {
                c_new_connection: rx_listener,
            };
            c_listeners.push(tx_listener);
            out.push((dialer, listener));
        }

        (
            ConnectionMaker {
                c_dialers,
                c_listeners,
            },
            out,
        )
    }

    pub async fn run(&mut self) {
        loop {
            info!(target: "validator-network", "ConnectionMaker: waiting for new request...");
            let (addr, c) = self.c_dialers.next().await.expect("should receive");
            info!(target: "validator-network", "ConnectionMaker: received request");
            let (l_stream, r_stream) = Connection::new(4096);
            info!(target: "validator-network", "ConnectionMaker: sending stream");
            c.send(l_stream).expect("should send");
            self.c_listeners[addr as usize]
                .unbounded_send(r_stream)
                .expect("should send");
        }
    }
}

#[derive(Clone)]
pub struct Dialer {
    c_connect: mpsc::UnboundedSender<(Multiaddress, oneshot::Sender<Connection>)>,
}

#[async_trait::async_trait]
impl DialerT<Multiaddress> for Dialer {
    type Connection = Connection;
    type Error = std::io::Error;

    async fn connect(
        &mut self,
        addresses: Vec<Multiaddress>,
    ) -> Result<Self::Connection, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.c_connect
            .unbounded_send((addresses[0], tx))
            .expect("should send");
        Ok(rx.await.expect("should receive"))
    }
}

pub struct Listener {
    c_new_connection: mpsc::UnboundedReceiver<Connection>,
}

#[async_trait::async_trait]
impl ListenerT for Listener {
    type Connection = Connection;
    type Error = std::io::Error;

    async fn accept(&mut self) -> Result<Self::Connection, Self::Error> {
        Ok(self.c_new_connection.next().await.expect("should receive"))
    }
}

#[cfg(test)]
mod tests {
    // use futures::{join, try_join};
    use std::collections::HashMap;

    use aleph_primitives::AuthorityId;
    use env_logger;
    use futures::{channel::oneshot, future::join_all};
    use rand::{thread_rng, Rng};
    use sc_service::TaskManager;
    use tokio::{runtime::Handle, time::Duration};
    use sc_service::SpawnTaskHandle;
    use super::{keys, ConnectionMaker, Dialer, Listener, Multiaddress};
    use crate::{
        crypto::AuthorityPen,
        validator_network::{service::Service, Network},
    };

    type Data = u32;

    fn f(
        pen: AuthorityPen,
        addr: HashMap<AuthorityId, Vec<Multiaddress>>,
        n_msg: usize,
        dialer: Dialer,
        listener: Listener,
        spawn_handle: SpawnTaskHandle,
    ) -> oneshot::Receiver<()> {
        let (service, mut interface) = Service::new(dialer, listener, pen, spawn_handle);

        let mut ids = vec![];
        for (id, addrs) in addr.into_iter() {
            interface.add_connection(id.clone(), addrs);
            ids.push(id);
        }

        tokio::spawn(async {
            let (_exit, rx) = oneshot::channel();
            service.run(rx).await;
        });

        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let send_timeout = Duration::from_millis(500);
            let mut received: Vec<bool> = vec![false; n_msg];
            let mut send_ticker = tokio::time::interval(send_timeout);
            let n_peers = ids.len();
            loop {
                tokio::select! {
                    _ = send_ticker.tick() => {
                        let data: Data = thread_rng().gen_range(0..n_msg) as u32;
                        let peer: AuthorityId = ids[thread_rng().gen_range(0..n_peers)].clone();
                        interface.send(data, peer);
                    },
                    data = interface.next() => {
                        let data: Data = data.expect("next should not be closed");
                        received[data as usize] = true;
                        if received.iter().all(|x| *x) {
                            tx.send(()).expect("should send");
                            break;
                        } else {
                            println!("Received {}/{} so far", received.iter().filter(|x| **x).count(), n_msg);
                        }
                    },
                };
            }
            loop {
                let data: Data = thread_rng().gen_range(0..n_msg) as u32;
                let peer: AuthorityId = ids[thread_rng().gen_range(0..n_peers)].clone();
                interface.send(data, peer);
                tokio::time::sleep(send_timeout).await;
            }
        });
        rx
    }

    #[tokio::test]
    async fn integration() {
        env_logger::init();
        let n_peers = 5;
        let n_msg = 20;
        let task_manager = TaskManager::new(Handle::current(), None).expect("should create TaskManager");
        let spawn_handle = task_manager.spawn_handle();
        let (mut cm, out) = ConnectionMaker::new(n_peers);
        tokio::spawn(async move {
            cm.run().await;
        });
        let mut x = vec![];
        for _ in 0..n_peers {
            x.push(keys().await);
        }
        let (ids, pens): (Vec<AuthorityId>, Vec<AuthorityPen>) = x.into_iter().unzip();
        let addr: HashMap<AuthorityId, Vec<Multiaddress>> = ids
            .clone()
            .into_iter()
            .zip(0..n_peers)
            .map(|(id, u)| (id, vec![u as u32]))
            .collect();
        let ready: Vec<oneshot::Receiver<()>> = out
            .into_iter()
            .zip(pens.into_iter())
            .map(|((dialer, listener), pen)| f(pen, addr.clone(), n_msg, dialer, listener, spawn_handle.clone()))
            .collect();
        let _result = join_all(ready).await;
    }
}
