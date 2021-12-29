use crate::new_network::{Network, NetworkEventStream, NetworkSendError, PeerId, RequestBlocks};
use async_trait::async_trait;
use futures::channel::{mpsc, oneshot};
use parking_lot::Mutex;
use sc_network::{Event, Multiaddr};
use sp_api::NumberFor;
use sp_runtime::traits::Block;
use std::{
    borrow::Cow,
    collections::{HashSet, VecDeque},
    sync::Arc,
};

type Channel<T> = (
    Arc<Mutex<mpsc::UnboundedSender<T>>>,
    Arc<Mutex<mpsc::UnboundedReceiver<T>>>,
);

fn channel<T>() -> Channel<T> {
    let (tx, rx) = mpsc::unbounded();
    (Arc::new(Mutex::new(tx)), Arc::new(Mutex::new(rx)))
}

#[derive(Clone)]
pub struct MockNetwork<B: Block> {
    pub request_justification: Channel<(B::Hash, NumberFor<B>)>,
    pub request_stale_block: Channel<(B::Hash, NumberFor<B>)>,
    pub add_reserved: Channel<(HashSet<Multiaddr>, Cow<'static, str>)>,
    pub remove_reserved: Channel<(HashSet<PeerId>, Cow<'static, str>)>,
    pub send_message: Channel<(Vec<u8>, PeerId, Cow<'static, str>)>,
    pub event_sinks: Arc<Mutex<Vec<mpsc::UnboundedSender<Event>>>>,
    event_stream_taken_oneshot: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pub network_errors: Arc<Mutex<VecDeque<NetworkSendError>>>,
}

impl<B: Block> RequestBlocks<B> for MockNetwork<B> {
    fn request_justification(&self, hash: &B::Hash, number: NumberFor<B>) {
        self.request_justification
            .0
            .lock()
            .unbounded_send((*hash, number))
            .unwrap();
    }

    fn request_stale_block(&self, hash: B::Hash, number: NumberFor<B>) {
        self.request_stale_block
            .0
            .lock()
            .unbounded_send((hash, number))
            .unwrap();
    }
}

#[async_trait]
impl<B: Block> Network for MockNetwork<B> {
    fn event_stream(&self) -> NetworkEventStream {
        let (tx, rx) = mpsc::unbounded();
        self.event_sinks.lock().push(tx);
        // Necessary for tests to detect when service takes event_stream
        if let Some(tx) = self.event_stream_taken_oneshot.lock().take() {
            tx.send(()).unwrap();
        }
        Box::pin(rx)
    }

    async fn send<'a>(
        &'a self,
        data: impl Into<Vec<u8>> + Send + Sync + 'static,
        peer_id: PeerId,
        protocol: Cow<'static, str>,
    ) -> Result<(), NetworkSendError> {
        if let Some(err) = self.network_errors.lock().pop_front() {
            Err(err)
        } else {
            self.send_message
                .0
                .lock()
                .unbounded_send((data.into(), peer_id, protocol))
                .unwrap();
            Ok(())
        }
    }

    fn add_reserved(&self, addresses: HashSet<Multiaddr>, protocol: Cow<'static, str>) {
        self.add_reserved
            .0
            .lock()
            .unbounded_send((addresses, protocol))
            .unwrap();
    }

    fn remove_reserved(&self, peers: HashSet<PeerId>, protocol: Cow<'static, str>) {
        self.remove_reserved
            .0
            .lock()
            .unbounded_send((peers, protocol))
            .unwrap();
    }
}

impl<B: Block> MockNetwork<B> {
    pub fn new(oneshot_sender: oneshot::Sender<()>) -> Self {
        MockNetwork {
            request_justification: channel(),
            request_stale_block: channel(),
            add_reserved: channel(),
            remove_reserved: channel(),
            send_message: channel(),
            event_sinks: Arc::new(Mutex::new(vec![])),
            event_stream_taken_oneshot: Arc::new(Mutex::new(Some(oneshot_sender))),
            network_errors: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn emit_event(&mut self, event: Event) {
        for sink in &*self.event_sinks.lock() {
            sink.unbounded_send(event.clone()).unwrap();
        }
    }

    // Consumes the network asserting there are no unreceived messages in the channels.
    pub fn close_channels(self) {
        self.event_sinks.lock().clear();

        self.request_justification.0.lock().close_channel();
        assert!(self
            .request_justification
            .1
            .lock()
            .try_next()
            .unwrap()
            .is_none());
        self.request_stale_block.0.lock().close_channel();
        assert!(self
            .request_stale_block
            .1
            .lock()
            .try_next()
            .unwrap()
            .is_none());
        self.add_reserved.0.lock().close_channel();
        assert!(self.add_reserved.1.lock().try_next().unwrap().is_none());
        self.remove_reserved.0.lock().close_channel();
        assert!(self.remove_reserved.1.lock().try_next().unwrap().is_none());
        self.send_message.0.lock().close_channel();
        assert!(self.send_message.1.lock().try_next().unwrap().is_none());
    }
}
