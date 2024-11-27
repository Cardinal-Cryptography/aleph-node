use std::{
    borrow::BorrowMut,
    collections::{hash_map::Entry, HashMap, HashSet},
    fmt::Display,
    num::NonZeroUsize,
    sync::Arc,
};

use futures::{channel::mpsc, Future, SinkExt, StreamExt};
use log::{debug, error, info, warn};
use lru::LruCache;
use network_clique::SpawnHandleT;
use sc_network::{MessageSink, PeerId};

use crate::{
    network::{Data, GossipNetwork},
    ProtocolNetwork,
};

const LOG_TARGET: &str = "aleph-sync-flooder";

struct FloodingProtocolNetwork<Flooder = ()> {
    protocol_network: ProtocolNetwork,
    flooder: Flooder,
}

impl FloodingProtocolNetwork {
    fn new_for_receiver(
        protocol_network: ProtocolNetwork,
        mut peer_filter: impl FnMut(&PeerId) -> bool + Send + 'static,
    ) -> (
        FloodingProtocolNetwork<impl FnMut(Vec<u8>, PeerId, &mut ProtocolNetwork) + Send + 'static>,
        mpsc::Receiver<(Vec<u8>, PeerId, Arc<Box<dyn MessageSink>>)>,
    ) {
        let (mut flooder_sender, flooder_receiver) = mpsc::channel(0);
        let mut filtered_peers = HashMap::new();
        let network = FloodingProtocolNetwork {
            protocol_network,
            flooder: move |data, peer_id, protocol_network: &mut ProtocolNetwork| {
                if !peer_filter(&peer_id) {
                    filtered_peers.remove(&peer_id);
                    return;
                }
                let message_sink = match filtered_peers.entry(peer_id) {
                    Entry::Vacant(vacant) => {
                        let network_service: &Box<dyn sc_network::config::NotificationService> =
                            protocol_network.borrow_mut();
                        let Some(message_sink) = network_service.message_sink(&peer_id) else {
                            return;
                        };
                        vacant.insert(Arc::new(message_sink)).clone()
                    }
                    Entry::Occupied(message_sink) => message_sink.get().clone(),
                };
                if let Err(true) = flooder_sender
                    .try_send((data, peer_id, message_sink))
                    .map_err(|err| err.is_disconnected())
                {
                    panic!("Flooder unexpectedly died.");
                }
            },
        };
        (network, flooder_receiver)
    }

    pub fn new<SH>(
        protocol_network: ProtocolNetwork,
        peer_filter: impl FnMut(&PeerId) -> bool + Send + 'static,
        spawn_handle: SH,
    ) -> (
        FloodingProtocolNetwork<impl FnMut(Vec<u8>, PeerId, &mut ProtocolNetwork) + Send + 'static>,
        impl Future<Output = Result<(), &'static str>> + Send,
    )
    where
        SH: SpawnHandleT + Send,
    {
        let (network, flooder_receiver) = Self::new_for_receiver(protocol_network, peer_filter);
        let flooder = MultiPeerNetworkSendFlooder::new(spawn_handle, flooder_receiver).run();
        (network, flooder)
    }
}

#[async_trait::async_trait]
impl<Flooder: FnMut(Vec<u8>, PeerId, &mut ProtocolNetwork) + Send + 'static, D: Data>
    GossipNetwork<D> for FloodingProtocolNetwork<Flooder>
{
    type Error = <ProtocolNetwork as GossipNetwork<D>>::Error;
    type PeerId = <ProtocolNetwork as GossipNetwork<D>>::PeerId;

    fn send_to(&mut self, data: D, peer_id: Self::PeerId) -> Result<(), Self::Error> {
        (self.flooder)(data.encode(), peer_id.clone(), &mut self.protocol_network);
        self.protocol_network.send_to(data, peer_id)
    }

    fn send_to_random(
        &mut self,
        data: D,
        peer_ids: HashSet<Self::PeerId>,
    ) -> Result<(), Self::Error> {
        self.protocol_network.send_to_random(data, peer_ids)
    }

    fn broadcast(&mut self, data: D) -> Result<(), Self::Error> {
        self.protocol_network.broadcast(data)
    }

    async fn next(&mut self) -> Result<(D, PeerId), Self::Error> {
        self.protocol_network.next().await
    }
}

struct NetworkSendFlooder<Data, NetworkSender> {
    data: futures::channel::mpsc::Receiver<(Data, NetworkSender)>,
}

impl<Data, NetworkSender> NetworkSendFlooder<Data, NetworkSender> {
    pub fn new(receiver: futures::channel::mpsc::Receiver<(Data, NetworkSender)>) -> Self {
        Self { data: receiver }
    }
}

pub trait NetworkSender<D> {
    type Error;

    async fn send(&mut self, data: &D) -> Result<(), Self::Error>;
}

impl<'a> NetworkSender<Vec<u8>> for Arc<Box<dyn MessageSink + 'a>> {
    type Error = sc_network::error::Error;

    async fn send(&mut self, data: &Vec<u8>) -> Result<(), Self::Error> {
        self.send_async_notification(data.clone()).await
    }
}

impl<Data, NS> NetworkSendFlooder<Data, NS> {
    pub async fn run(mut self) -> Result<(), String>
    where
        NS: NetworkSender<Data>,
        <NS as NetworkSender<Data>>::Error: Display,
    {
        // we try to send as many copies as we can  of some valid message
        let (mut message, mut sink) = self
            .data
            .next()
            .await
            .ok_or_else(|| "data stream was closed")?;
        loop {
            sink.send(&message)
                .await
                .map_err(|err| format!("Error while sending in flooder: {}.", err))?;

            (message, sink) = match self.data.try_next() {
                Ok(Some(new_message)) => new_message,
                Ok(None) => return Err("data stream was suddenly closed")?,
                Err(_) => continue,
            };
        }
    }
}

struct MultiPeerNetworkSendFlooder<SH> {
    data: futures::channel::mpsc::Receiver<(Vec<u8>, PeerId, Arc<Box<dyn MessageSink>>)>,
    spawn_handle: SH,
    peers: LruCache<PeerId, mpsc::Sender<(Vec<u8>, Arc<Box<dyn MessageSink>>)>>,
}

impl<SH> MultiPeerNetworkSendFlooder<SH> {
    pub fn new(
        spawn_handle: SH,
        data_receiver: mpsc::Receiver<(Vec<u8>, PeerId, Arc<Box<dyn MessageSink>>)>,
    ) -> Self {
        Self {
            data: data_receiver,
            spawn_handle,
            peers: LruCache::new(NonZeroUsize::new(10).expect("10 is greater than 0. qed")),
        }
    }
}

impl<SH: SpawnHandleT + Send> MultiPeerNetworkSendFlooder<SH> {
    pub async fn run(mut self) -> Result<(), &'static str> {
        while let Some((data, peer_id, sink)) = self.data.next().await {
            let sender = self.peers.get_or_insert_mut(peer_id, || {
                let (flooder_sender, flooder_receiver) = mpsc::channel(0);
                self.spawn_handle.spawn("flooder-{peer_id}", async move {
                    if let Err(err) = NetworkSendFlooder::new(flooder_receiver).run().await {
                        debug!(target: LOG_TARGET, "Some `NetworkSendFlooder` returned an err: {err}");
                    }
                });
                flooder_sender
            });
            if let Err(err) = sender.send((data, sink)).await {
                warn!(target: LOG_TARGET, "Unable to send data to instance of [`NetworkSendFlooder`]: {err}. Removing peer...");
                self.peers.pop(&peer_id);
            }
        }
        warn!(target: LOG_TARGET, "Multi-node flooder exited too early.");
        Ok(())
    }
}

pub fn initialize_network_flooding<SH, D>(
    block_sync_network: crate::ProtocolNetwork,
    spawn_handle: &SH,
) -> impl GossipNetwork<D>
where
    SH: SpawnHandleT + Send + Clone + 'static,
    D: Data,
{
    info!(target: LOG_TARGET, "Initialazing the network flooder.");
    let flooder_spawn_handle = spawn_handle.clone();
    let (block_sync_network, flooder) = FloodingProtocolNetwork::new(
        block_sync_network,
        move |peer_id| {
            info!(target: LOG_TARGET, "Sync-network flooder will flood {peer_id} with messages.");
            return true;
        },
        flooder_spawn_handle,
    );

    spawn_handle.spawn("sync-network-flooder", async move {
        if let Err(err) = flooder.await {
            error!(
                target: LOG_TARGET,
                "Flooder unexpectedly finished with error: {err}."
            );
        }
    });
    block_sync_network
}
