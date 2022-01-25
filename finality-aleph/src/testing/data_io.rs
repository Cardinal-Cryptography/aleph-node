use crate::data_io::{AlephData, AlephDataFor, AlephNetworkMessage, DataStore, DataStoreConfig};
use crate::network::{ComponentNetwork, DataNetwork, RequestBlocks};
use aleph_bft::Recipient;
use async_trait::async_trait;
use futures::{
    channel::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    StreamExt,
};
use sc_block_builder::BlockBuilderProvider;
use sp_api::BlockId;
use sp_api::NumberFor;
use sp_consensus::BlockOrigin;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::Digest;
use std::{default::Default, future::Future, sync::Arc, time::Duration};
use substrate_test_runtime_client::{
    runtime::Block, Backend, ClientBlockImportExt, DefaultTestClientBuilderExt, TestClient,
    TestClientBuilder, TestClientBuilderExt,
};
use tokio::sync::Mutex;

#[derive(Clone)]
struct TestBlockRequester<B: BlockT> {
    blocks: mpsc::UnboundedSender<AlephDataFor<B>>,
    justifications: mpsc::UnboundedSender<AlephDataFor<B>>,
}

impl<B: BlockT> TestBlockRequester<B> {
    fn new() -> (
        Self,
        UnboundedReceiver<AlephDataFor<B>>,
        UnboundedReceiver<AlephDataFor<B>>,
    ) {
        let (blocks_tx, blocks_rx) = mpsc::unbounded();
        let (justifications_tx, justifications_rx) = mpsc::unbounded();
        (
            TestBlockRequester {
                blocks: blocks_tx,
                justifications: justifications_tx,
            },
            blocks_rx,
            justifications_rx,
        )
    }
}

impl<B: BlockT> RequestBlocks<B> for TestBlockRequester<B> {
    fn request_justification(&self, hash: &B::Hash, number: NumberFor<B>) {
        self.justifications
            .unbounded_send(AlephData {
                hash: *hash,
                number,
            })
            .unwrap();
    }

    fn request_stale_block(&self, hash: B::Hash, number: NumberFor<B>) {
        self.blocks
            .unbounded_send(AlephData { hash, number })
            .unwrap();
    }
}

type TestData = Vec<AlephDataFor<Block>>;

impl AlephNetworkMessage<Block> for TestData {
    fn included_blocks(&self) -> Vec<AlephDataFor<Block>> {
        self.clone()
    }
}

#[derive(Clone)]
struct TestNetwork {
    sender_tx: UnboundedSender<(TestData, Recipient)>,
    next_rx: Arc<Mutex<UnboundedReceiver<TestData>>>,
}

impl TestNetwork {
    fn new() -> (
        Self,
        UnboundedReceiver<(TestData, Recipient)>,
        UnboundedSender<TestData>,
    ) {
        let (sender_tx, sender_rx) = mpsc::unbounded();
        let (next_tx, next_rx) = mpsc::unbounded();
        (
            TestNetwork {
                sender_tx,
                next_rx: Arc::new(Mutex::new(next_rx)),
            },
            sender_rx,
            next_tx,
        )
    }
}

#[async_trait]
impl ComponentNetwork<TestData> for TestNetwork {
    type S = UnboundedSender<(TestData, Recipient)>;
    type R = UnboundedReceiver<TestData>;

    fn sender(&self) -> &Self::S {
        &self.sender_tx
    }

    fn receiver(&self) -> Arc<Mutex<Self::R>> {
        self.next_rx.clone()
    }
}

struct DataStoreChannels {
    network: Box<dyn DataNetwork<TestData>>,
    block_requests_rx: UnboundedReceiver<AlephDataFor<Block>>,
    network_tx: UnboundedSender<TestData>,
    exit_data_store_tx: oneshot::Sender<()>,
}

fn prepare_data_store() -> (impl Future<Output = ()>, Arc<TestClient>, DataStoreChannels) {
    let client = Arc::new(TestClientBuilder::new().build());

    let (block_requester, block_requests_rx, _justification_requests_rx) =
        TestBlockRequester::new();
    let (test_network, _network_rx, network_tx) = TestNetwork::new();
    let data_store_config = DataStoreConfig {
        available_blocks_cache_capacity: 1000,
        message_id_boundary: 100_000,
        periodic_maintenance_interval: Duration::from_millis(30),
        request_block_after: Duration::from_millis(50),
    };

    let (mut data_store, network) = DataStore::<
        Block,
        TestClient,
        Backend,
        TestBlockRequester<Block>,
        TestData,
        mpsc::UnboundedReceiver<TestData>,
    >::new(
        client.clone(),
        block_requester,
        data_store_config,
        test_network,
    );
    let (exit_data_store_tx, exit_data_store_rx) = oneshot::channel();

    (
        async move {
            data_store.run(exit_data_store_rx).await;
        },
        client,
        DataStoreChannels {
            network: Box::new(network),
            block_requests_rx,
            network_tx,
            exit_data_store_tx,
        },
    )
}

async fn import_blocks(
    client: &mut Arc<TestClient>,
    n: u32,
    finalize: bool,
) -> Vec<AlephDataFor<Block>> {
    let mut blocks = vec![];
    for _ in 0..n {
        let block = client
            .new_block(Default::default())
            .unwrap()
            .build()
            .unwrap()
            .block;
        if finalize {
            client
                .import_as_final(BlockOrigin::Own, block.clone())
                .await
                .unwrap();
        } else {
            client
                .import(BlockOrigin::Own, block.clone())
                .await
                .unwrap();
        }
        blocks.push(AlephData::new(block.header.hash(), block.header.number));
    }
    blocks
}

#[tokio::test]
async fn sends_messages_with_imported_blocks() {
    let (
        task_handle,
        mut client,
        DataStoreChannels {
            mut network,
            block_requests_rx: _,
            network_tx,
            exit_data_store_tx,
        },
    ) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);

    let blocks = import_blocks(&mut client, 4, false).await;

    network_tx.unbounded_send(blocks.clone()).unwrap();

    let message = network.next().await.unwrap();
    assert_eq!(message.included_blocks(), blocks);

    exit_data_store_tx.send(()).unwrap();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn sends_messages_after_import() {
    let (
        task_handle,
        mut client,
        DataStoreChannels {
            mut network,
            block_requests_rx: _,
            network_tx,
            exit_data_store_tx,
        },
    ) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);

    // Build block
    let block = client
        .new_block(Default::default())
        .unwrap()
        .build()
        .unwrap()
        .block;

    let data = AlephData::new(block.header.hash(), block.header.number);

    // Send created block
    network_tx.unbounded_send(vec![data]).unwrap();

    // Import created block
    client
        .import(BlockOrigin::Own, block.clone())
        .await
        .unwrap();

    // Block should be sent to network
    let message = network.next().await.unwrap();
    assert_eq!(message.included_blocks(), vec![data]);

    exit_data_store_tx.send(()).unwrap();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn sends_messages_with_number_lower_than_finalized() {
    let (
        task_handle,
        mut client,
        DataStoreChannels {
            mut network,
            block_requests_rx: _,
            network_tx,
            exit_data_store_tx,
        },
    ) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);

    // Finalize 4 blocks
    import_blocks(&mut client, 4, true).await;

    // Build 4 new blocks different from finalized
    let mut blocks = Vec::new();
    for i in 0u64..4 {
        let mut digest = Digest::default();
        digest.push(sp_runtime::generic::DigestItem::Other(
            i.to_le_bytes().to_vec(),
        ));
        let block = client
            .new_block_at(&BlockId::Number(i), digest, false)
            .unwrap()
            .build()
            .unwrap()
            .block;
        blocks.push(AlephData::new(block.header.hash(), block.header.number));
    }

    // Send the created blocks
    network_tx.unbounded_send(blocks.clone()).unwrap();

    // Blocks should be sent to network
    let message = network.next().await.unwrap();
    assert_eq!(message.included_blocks(), blocks);

    exit_data_store_tx.send(()).unwrap();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn does_not_send_messages_without_import() {
    let (
        task_handle,
        mut client,
        DataStoreChannels {
            mut network,
            block_requests_rx: _,
            network_tx,
            exit_data_store_tx,
        },
    ) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);

    // Import 4 blocks
    let blocks = import_blocks(&mut client, 4, false).await;

    // Build new block
    let not_imported_block = client
        .new_block(Default::default())
        .unwrap()
        .build()
        .unwrap()
        .block;

    // Send new block
    network_tx
        .unbounded_send(vec![AlephData::new(
            not_imported_block.header.hash(),
            not_imported_block.header.number,
        )])
        .unwrap();

    // Send imported blocks
    network_tx.unbounded_send(blocks.clone()).unwrap();

    // Should receive imported blocks
    let message = network.next().await.unwrap();
    assert_eq!(message.included_blocks(), blocks);

    // Close Data Store
    exit_data_store_tx.send(()).unwrap();

    // Network should have no other block than already imported
    let message = network.next().await;
    assert!(message.is_none());

    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn sends_block_request_on_missing_block() {
    let (
        task_handle,
        client,
        DataStoreChannels {
            network: _,
            mut block_requests_rx,
            network_tx,
            exit_data_store_tx,
        },
    ) = prepare_data_store();

    let data_store_handle = tokio::spawn(task_handle);

    // Build new block
    let block = client
        .new_block(Default::default())
        .unwrap()
        .build()
        .unwrap()
        .block;
    let data = AlephData::new(block.header.hash(), block.header.number);

    // Send new block
    network_tx.unbounded_send(vec![data]).unwrap();

    // New block should be requested
    let requested_block = block_requests_rx.next().await.unwrap();
    assert_eq!(requested_block, data);

    exit_data_store_tx.send(()).unwrap();
    data_store_handle.await.unwrap();
}
