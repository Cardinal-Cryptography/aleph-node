use super::*;
use crate::KEY_TYPE;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use rush::NodeIndex;
use sc_network::{Event, ObservedRole, PeerId, ReputationChange};
use sp_keystore::{testing::KeyStore, CryptoStore};
use sp_runtime::traits::Block as BlockT;
use std::collections::HashSet;
use substrate_test_runtime::{Block, Hash};

type Channel<T> = (
    Arc<Mutex<mpsc::UnboundedSender<T>>>,
    Arc<Mutex<mpsc::UnboundedReceiver<T>>>,
);

fn channel<T>() -> Channel<T> {
    let (tx, rx) = mpsc::unbounded();
    (Arc::new(Mutex::new(tx)), Arc::new(Mutex::new(rx)))
}

struct TestNetwork<B: BlockT> {
    event_sinks: Arc<Mutex<Vec<mpsc::UnboundedSender<Event>>>>,
    oneshot_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    report_peer: Channel<(PeerId, ReputationChange)>,
    disconnect_peer: Channel<(PeerId, Cow<'static, str>)>,
    send_message: Channel<(PeerId, Cow<'static, str>, Vec<u8>)>,
    announce: Channel<(B::Hash, Option<Vec<u8>>)>,
    add_set_reserved: Channel<(PeerId, Cow<'static, str>)>,
    remove_set_reserved: Channel<(PeerId, Cow<'static, str>)>,
}

impl<B: BlockT> TestNetwork<B> {
    fn new(tx: oneshot::Sender<()>) -> Self {
        TestNetwork {
            event_sinks: Arc::new(Mutex::new(vec![])),
            oneshot_sender: Arc::new(Mutex::new(Some(tx))),
            report_peer: channel(),
            disconnect_peer: channel(),
            send_message: channel(),
            announce: channel(),
            add_set_reserved: channel(),
            remove_set_reserved: channel(),
        }
    }
}
impl<B: BlockT> Clone for TestNetwork<B> {
    fn clone(&self) -> Self {
        TestNetwork {
            event_sinks: self.event_sinks.clone(),
            oneshot_sender: self.oneshot_sender.clone(),
            report_peer: self.report_peer.clone(),
            disconnect_peer: self.disconnect_peer.clone(),
            send_message: self.send_message.clone(),
            announce: self.announce.clone(),
            add_set_reserved: self.add_set_reserved.clone(),
            remove_set_reserved: self.remove_set_reserved.clone(),
        }
    }
}

impl<B: BlockT> Network<B> for TestNetwork<B> {
    fn event_stream(&self) -> Pin<Box<dyn Stream<Item = Event> + Send>> {
        let (tx, rx) = mpsc::unbounded();
        self.event_sinks.lock().push(tx);
        if let Some(tx) = self.oneshot_sender.lock().take() {
            tx.send(()).unwrap();
        }
        Box::pin(rx)
    }

    fn _report_peer(&self, peer_id: PeerId, reputation: ReputationChange) {
        self.report_peer
            .0
            .lock()
            .unbounded_send((peer_id, reputation))
            .unwrap();
    }

    fn _disconnect_peer(&self, peer_id: PeerId, protocol: Cow<'static, str>) {
        self.disconnect_peer
            .0
            .lock()
            .unbounded_send((peer_id, protocol))
            .unwrap();
    }

    fn send_message(&self, peer_id: PeerId, protocol: Cow<'static, str>, message: Vec<u8>) {
        self.send_message
            .0
            .lock()
            .unbounded_send((peer_id, protocol, message))
            .unwrap();
    }

    fn _announce(&self, block: <B as BlockT>::Hash, associated_data: Option<Vec<u8>>) {
        self.announce
            .0
            .lock()
            .unbounded_send((block, associated_data))
            .unwrap();
    }

    fn add_set_reserved(&self, who: PeerId, protocol: Cow<'static, str>) {
        self.add_set_reserved
            .0
            .lock()
            .unbounded_send((who, protocol))
            .unwrap();
    }

    fn remove_set_reserved(&self, who: PeerId, protocol: Cow<'static, str>) {
        self.remove_set_reserved
            .0
            .lock()
            .unbounded_send((who, protocol))
            .unwrap();
    }
}

impl<B: BlockT> TestNetwork<B> {
    fn emit_event(&self, event: Event) {
        for sink in &*self.event_sinks.lock() {
            sink.unbounded_send(event.clone()).unwrap();
        }
    }

    // Consumes the network asserting there are no unreceived messages in the channels.
    fn close_channels(self) {
        self.event_sinks.lock().clear();
        self.report_peer.0.lock().close_channel();
        assert!(self.report_peer.1.lock().try_next().unwrap().is_none());
        self.disconnect_peer.0.lock().close_channel();
        assert!(self.disconnect_peer.1.lock().try_next().unwrap().is_none());
        self.send_message.0.lock().close_channel();
        assert!(self.send_message.1.lock().try_next().unwrap().is_none());
        self.announce.0.lock().close_channel();
        assert!(self.announce.1.lock().try_next().unwrap().is_none());
        self.add_set_reserved.0.lock().close_channel();
        assert!(self.add_set_reserved.1.lock().try_next().unwrap().is_none());
        self.remove_set_reserved.0.lock().close_channel();
        assert!(self
            .remove_set_reserved
            .1
            .lock()
            .try_next()
            .unwrap()
            .is_none());
    }
}

struct Authority {
    id: AuthorityId,
    peer_id: PeerId,
}

async fn generate_authority(s: &str) -> Authority {
    let key_store = Arc::new(KeyStore::new());
    let pk = key_store
        .sr25519_generate_new(KEY_TYPE, Some(s))
        .await
        .unwrap();
    assert_eq!(key_store.keys(KEY_TYPE).await.unwrap().len(), 3);
    let id = AuthorityId::from(pk);
    let peer_id = PeerId::random();
    Authority { id, peer_id }
}

struct TestData {
    network: TestNetwork<Block>,
    _alice: Authority,
    bob: Authority,
    charlie: Authority,
    command_tx: mpsc::UnboundedSender<NetworkCommand<Block, Hash>>,
    event_rx: mpsc::UnboundedReceiver<NetworkEvent<Block, Hash>>,
    consensus_network_handle: tokio::task::JoinHandle<()>,
}

impl TestData {
    // consumes the test data asserting there are no unread messages in the channels
    // and awaits for the consensus_network task.
    async fn complete(mut self) {
        self.network.close_channels();
        self.command_tx.close_channel();
        assert!(self.event_rx.try_next().is_err());
        self.consensus_network_handle.await.unwrap();
    }
}

const PROTOCOL_NAME: &str = "/test/1";

async fn prepare_one_epoch_test_data() -> TestData {
    let (oneshot_tx, oneshot_rx) = oneshot::channel();
    let network = TestNetwork::<Block>::new(oneshot_tx);
    let consensus_network =
        ConsensusNetwork::<Block, Hash, TestNetwork<Block>>::new(network.clone(), PROTOCOL_NAME);

    let _alice = generate_authority("//Alice").await;
    let bob = generate_authority("//Bob").await;
    let charlie = generate_authority("//Charlie").await;

    let authorities: Vec<_> = [&_alice, &bob, &charlie]
        .iter()
        .map(|auth| auth.id.clone())
        .collect();

    let event_rx = consensus_network.start_epoch(EpochId(0), authorities);
    let (command_tx, command_rx) = mpsc::unbounded();
    let consensus_network_handle =
        tokio::spawn(async move { consensus_network.run(command_rx).await });

    // wait till consensus_network takes the event_stream
    oneshot_rx.await.unwrap();

    TestData {
        network,
        _alice,
        bob,
        charlie,
        command_tx,
        event_rx,
        consensus_network_handle,
    }
}

#[tokio::test]
async fn test_network_event_sync_connnected() {
    let data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::SyncConnected {
        remote: data.bob.peer_id,
    });
    let (peer_id, protocol) = data.network.add_set_reserved.1.lock().next().await.unwrap();
    assert_eq!(peer_id, data.bob.peer_id);
    assert_eq!(protocol, PROTOCOL_NAME);
    data.complete().await;
}
#[tokio::test]
async fn test_network_event_sync_disconnected() {
    let data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::SyncDisconnected {
        remote: data.charlie.peer_id,
    });
    let (peer_id, protocol) = data
        .network
        .remove_set_reserved
        .1
        .lock()
        .next()
        .await
        .unwrap();
    assert_eq!(peer_id, data.charlie.peer_id);
    assert_eq!(protocol, PROTOCOL_NAME);
    data.complete().await;
}
#[tokio::test]
async fn test_network_event_notification_stream_opened() {
    let mut data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::NotificationStreamOpened {
        remote: data.bob.peer_id,
        protocol: PROTOCOL_NAME.into(),
        role: ObservedRole::Authority,
    });
    if let Some(NetworkEvent::PeerConnected(peer_id)) = data.event_rx.next().await {
        assert_eq!(peer_id, data.bob.peer_id);
    } else {
        panic!("Peer connected event did not arrive");
    }
    data.complete().await;
}
#[tokio::test]
async fn test_network_event_notification_stream_opened_incorrect_protocol() {
    let data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::NotificationStreamOpened {
        remote: data.bob.peer_id,
        protocol: "/incorrect/name".into(),
        role: ObservedRole::Authority,
    });
    data.complete().await;
}
#[tokio::test]
async fn test_network_event_notification_stream_closed() {
    let mut data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::NotificationStreamClosed {
        remote: data.bob.peer_id,
        protocol: PROTOCOL_NAME.into(),
    });
    if let Some(NetworkEvent::PeerDisconnected(peer_id)) = data.event_rx.next().await {
        assert_eq!(peer_id, data.bob.peer_id);
    } else {
        panic!("Peer disconnected event did not arrive");
    }
    data.complete().await;
}
#[tokio::test]
async fn test_network_event_notification_stream_closed_incorrect_protocol() {
    let data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::NotificationStreamClosed {
        remote: data.bob.peer_id,
        protocol: "/incorrect/name".into(),
    });
    data.complete().await;
}
#[tokio::test]
async fn test_network_event_notifications_received() {
    let consensus_message = ConsensusMessage::<Block, Hash>::RequestCoord((0, NodeIndex(0)).into());
    let network_message = NetworkMessage::Consensus(consensus_message, EpochId(0));
    let messages = vec![(PROTOCOL_NAME.into(), network_message.encode().into())];
    let mut data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::NotificationsReceived {
        remote: data.bob.peer_id,
        messages,
    });
    if let Some(NetworkEvent::MessageReceived(
        ConsensusMessage::<Block, Hash>::RequestCoord(unit_coord),
        peer_id,
    )) = data.event_rx.next().await
    {
        assert_eq!(unit_coord.creator.0, 0);
        assert_eq!(unit_coord.round, 0);
        assert_eq!(peer_id, data.bob.peer_id);
    }
    data.complete().await;
}

#[tokio::test]
async fn test_network_commands() {
    let mut data = prepare_one_epoch_test_data().await;
    data.network.emit_event(Event::NotificationStreamOpened {
        remote: data.bob.peer_id,
        protocol: PROTOCOL_NAME.into(),
        role: ObservedRole::Authority,
    });
    data.event_rx.next().await.unwrap();
    data.network.emit_event(Event::NotificationStreamOpened {
        remote: data.charlie.peer_id,
        protocol: PROTOCOL_NAME.into(),
        role: ObservedRole::Authority,
    });
    data.event_rx.next().await.unwrap();

    // SendToPeer
    {
        let network_message = NetworkMessage::Consensus(
            ConsensusMessage::RequestParents(Hash::from_low_u64_ne(7)),
            EpochId(0),
        );
        data.command_tx
            .send(NetworkCommand::SendToPeer(
                network_message,
                data.bob.peer_id,
            ))
            .await
            .unwrap();
        match data.network.send_message.1.lock().next().await {
            Some((peer_id, protocol, message)) => {
                assert_eq!(peer_id, data.bob.peer_id);
                assert_eq!(protocol, PROTOCOL_NAME);
                match NetworkMessage::<Block, Hash>::decode(&mut message.as_slice()) {
                    Ok(NetworkMessage::Consensus(
                        ConsensusMessage::RequestParents(hash),
                        epoch_id,
                    )) => {
                        assert_eq!(hash.to_low_u64_ne(), 7);
                        assert_eq!(epoch_id.0, 0);
                    }
                    _ => panic!("Expected a request parents consensus message"),
                }
            }
            _ => panic!("Expecting a network message"),
        }
    }

    // SendToAll
    {
        let network_message = NetworkMessage::Consensus(
            ConsensusMessage::RequestParents(Hash::from_low_u64_ne(8)),
            EpochId(0),
        );
        data.command_tx
            .send(NetworkCommand::SendToAll(network_message))
            .await
            .unwrap();
        let mut peer_ids = HashSet::<PeerId>::new();
        for _ in 0..2_u8 {
            match data.network.send_message.1.lock().next().await {
                Some((peer_id, protocol, message)) => {
                    peer_ids.insert(peer_id);
                    assert_eq!(protocol, PROTOCOL_NAME);
                    match NetworkMessage::<Block, Hash>::decode(&mut message.as_slice()) {
                        Ok(NetworkMessage::Consensus(
                            ConsensusMessage::RequestParents(hash),
                            epoch_id,
                        )) => {
                            assert_eq!(hash.to_low_u64_ne(), 8);
                            assert_eq!(epoch_id.0, 0);
                        }
                        _ => panic!("Expected a request parents consensus message"),
                    }
                }
                _ => panic!("Expecting two network messages"),
            }
        }
        let expected_peer_ids: HashSet<_> = [data.bob.peer_id, data.charlie.peer_id]
            .iter()
            .cloned()
            .collect();
        assert_eq!(peer_ids, expected_peer_ids);
    }

    // SendToRandPeer
    {
        let network_message = NetworkMessage::Consensus(
            ConsensusMessage::RequestParents(Hash::from_low_u64_ne(9)),
            EpochId(0),
        );
        data.command_tx
            .send(NetworkCommand::SendToRandPeer(network_message))
            .await
            .unwrap();
        match data.network.send_message.1.lock().next().await {
            Some((peer_id, protocol, message)) => {
                assert!(peer_id == data.bob.peer_id || peer_id == data.charlie.peer_id);
                assert_eq!(protocol, PROTOCOL_NAME);
                match NetworkMessage::<Block, Hash>::decode(&mut message.as_slice()) {
                    Ok(NetworkMessage::Consensus(
                        ConsensusMessage::RequestParents(hash),
                        epoch_id,
                    )) => {
                        assert_eq!(hash.to_low_u64_ne(), 9);
                        assert_eq!(epoch_id.0, 0);
                    }
                    _ => panic!("Expected a request parents consensus message"),
                }
            }
            _ => panic!("Expecting a network message"),
        }
    }

    // SendToRandPeer after bob disconnects
    {
        data.network.emit_event(Event::NotificationStreamClosed {
            remote: data.bob.peer_id,
            protocol: PROTOCOL_NAME.into(),
        });
        data.event_rx.next().await.unwrap();
        let network_message = NetworkMessage::Consensus(
            ConsensusMessage::RequestParents(Hash::from_low_u64_ne(10)),
            EpochId(0),
        );
        data.command_tx
            .send(NetworkCommand::SendToRandPeer(network_message))
            .await
            .unwrap();
        match data.network.send_message.1.lock().next().await {
            Some((peer_id, protocol, message)) => {
                assert_eq!(peer_id, data.charlie.peer_id);
                assert_eq!(protocol, PROTOCOL_NAME);
                match NetworkMessage::<Block, Hash>::decode(&mut message.as_slice()) {
                    Ok(NetworkMessage::Consensus(
                        ConsensusMessage::RequestParents(hash),
                        epoch_id,
                    )) => {
                        assert_eq!(hash.to_low_u64_ne(), 10);
                        assert_eq!(epoch_id.0, 0);
                    }
                    _ => panic!("Expected a request parents consensus message"),
                }
            }
            _ => panic!("Expecting a network message"),
        }
    }

    data.complete().await;
}
