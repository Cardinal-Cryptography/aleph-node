use crate::data_io::{MAX_DATA_BRANCH_LEN,
    AlephData, AlephNetworkMessage, AlephProposal, DataStore, DataStoreConfig,
    UnvalidatedAlephProposal,
};
use crate::network::{DataNetwork, RequestBlocks, SimpleNetwork};
use crate::session::{SessionBoundaries, SessionId, SessionPeriod};

use crate::BlockHashNum;
use futures::{
    channel::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    StreamExt,
};
use sc_block_builder::BlockBuilderProvider;
use sc_client_api::HeaderBackend;
use sp_api::BlockId;
use sp_api::NumberFor;
use sp_consensus::BlockOrigin;
pub use sp_core::hash::H256;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::Digest;
use std::{default::Default, future::Future, sync::Arc, time::Duration};
use substrate_test_runtime_client::{
    runtime::Block, Backend,  ClientExt,ClientBlockImportExt, DefaultTestClientBuilderExt, TestClient,
    TestClientBuilder, TestClientBuilderExt,
};

use tokio::time::timeout;

fn proposal_from_blocks(blocks: Vec<Block>) -> AlephProposal<Block> {
    let num = blocks.last().unwrap().header().number;
    let hashes = blocks.into_iter().map(|block| block.hash()).collect();
    AlephProposal::new(hashes, num)
}

fn unvalidated_proposal_from_blocks(blocks: Vec<Block>) -> UnvalidatedAlephProposal<Block> {
    let num = blocks.last().unwrap().header().number;
    let hashes = blocks.into_iter().map(|block| block.hash()).collect();
    UnvalidatedAlephProposal::new(hashes, num)
}


fn aleph_data_from_blocks(blocks: Vec<Block>) -> AlephData<Block> {
    if blocks.is_empty() {
        AlephData::Empty
    } else {
        AlephData::HeadProposal(unvalidated_proposal_from_blocks(blocks))
    }
}

#[derive(Clone)]
struct TestBlockRequester<B: BlockT> {
    blocks: UnboundedSender<BlockHashNum<B>>,
    justifications: UnboundedSender<BlockHashNum<B>>,
}

impl<B: BlockT> TestBlockRequester<B> {
    fn new() -> (
        Self,
        UnboundedReceiver<BlockHashNum<B>>,
        UnboundedReceiver<BlockHashNum<B>>,
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
            .unbounded_send((*hash, number).into())
            .unwrap();
    }

    fn request_stale_block(&self, hash: B::Hash, number: NumberFor<B>) {
        self.blocks.unbounded_send((hash, number).into()).unwrap();
    }

    fn clear_justification_requests(&self) {
        panic!("`clear_justification_requests` not implemented!")
    }
}

type TestData = Vec<AlephData<Block>>;

impl AlephNetworkMessage<Block> for TestData {
    fn included_data(&self) -> Vec<AlephData<Block>> {
        self.clone()
    }
}

struct TestHandler {
    client: Arc<TestClient>,
    // We need to have a second client, for the purpose of creating blocks only
    client_builder: Arc<TestClient>,
    block_requests_rx: UnboundedReceiver<BlockHashNum<Block>>,
    network_tx: UnboundedSender<TestData>,
    exit_data_store_tx: oneshot::Sender<()>,
    unique_seed: u32,
}

impl AlephData<Block> {
    fn from(block: Block) -> AlephData<Block> {
        //AlephData::new(block.header.hash(), block.header.number)
        AlephData::Empty
    }
}

impl TestHandler {
    /// Import block in test client
    async fn import_block(&mut self, block: Block, finalize: bool) {
        if finalize {
            self.client.import_as_final(BlockOrigin::Own, block.clone())
        } else {
            self.client.import(BlockOrigin::Own, block.clone())
        }
        .await
        .unwrap();
    }

    /// Finalize block with given hash without providing justification.
    fn finalize_block(&self, hash: &H256) {
        self.client.finalize_block(BlockId::Hash(*hash), None).unwrap();
    }


    fn genesis_hash(&mut self) -> H256 {
        assert_eq!(self.client.info().genesis_hash, self.client_builder.info().genesis_hash);
        self.client.info().genesis_hash
    }

    fn get_unique_bytes(&mut self) -> Vec<u8> {
        self.unique_seed += 1;
        self.unique_seed.to_be_bytes().to_vec()
    }

    // fn build_block_at_num(&mut self, num: u64) -> Block {
    //     let unique_bytes: Vec<u8> = self.get_unique_bytes();

    //     let mut digest = Digest::default();
    //     digest.push(sp_runtime::generic::DigestItem::Other(unique_bytes));
    //     self.client
    //         .new_block_at(&BlockId::Number(num), digest, false)
    //         .unwrap()
    //         .build()
    //         .unwrap()
    //         .block
    // }

    async fn build_block_at_hash(&mut self, hash: &H256) -> Block {
        let unique_bytes: Vec<u8> = self.get_unique_bytes();
        let mut digest = Digest::default();
        digest.push(sp_runtime::generic::DigestItem::Other(unique_bytes));
        let block = self.client_builder
            .new_block_at(&BlockId::Hash(hash.clone()), digest, false)
            .unwrap()
            .build()
            .unwrap()
            .block;

        self.client_builder.import(BlockOrigin::Own, block.clone()).await.unwrap();
        block

    }

    /// Builds a sequence of blocks extending from `hash` of length `len`
    async fn build_branch_upon(&mut self, hash: &H256, len: usize) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut prev_hash = *hash;
        for i in 0..len {
            let block = self.build_block_at_hash(&prev_hash).await;
            prev_hash = block.hash();
            self.client_builder.import(BlockOrigin::Own, block.clone());
            blocks.push(block);
        }
        blocks
    }

    /// imports a sequence of blocks, should be in correct order
    async fn import_branch(&mut self, blocks: Vec<Block>, finalize: bool)  {
        for block in blocks {
            self.import_block(block.clone(), finalize).await;
        }
    }


    /// Builds a sequence of blocks extending from `hash` of length `len` and imports them
    async fn build_and_import_branch_upon(&mut self, hash: &H256, len: usize, finalize: bool) -> Vec<Block> {
        let blocks = self.build_branch_upon(hash, len).await;
        self.import_branch(blocks.clone(), finalize).await;
        blocks
    }

    /// Build and import blocks in test client
    // async fn build_and_import_blocks(&mut self, n: u32, finalize: bool) -> TestData {
    //     let mut blocks = vec![];
    //     for _ in 0..n {
    //         let block = self.build_block();
    //         self.import_block(block.clone(), finalize).await;
    //         blocks.push(AlephData::from(block));
    //     }
    //     blocks
    // }

    /// Sends data to Data Store
    fn send_data(&self, data: TestData) {
        self.network_tx.unbounded_send(data).unwrap()
    }

    /// Exits Data Store
    fn exit(self) {
        self.exit_data_store_tx.send(()).unwrap();
    }

    /// Receive next block request from Data Store
    async fn next_block_request(&mut self) -> BlockHashNum<Block> {
        self.block_requests_rx.next().await.unwrap()
    }
}

fn prepare_data_store() -> (
    impl Future<Output = ()>,
    TestHandler,
    impl DataNetwork<TestData>,
) {
    let client = Arc::new(TestClientBuilder::new().build());
    let client_builder = Arc::new(TestClientBuilder::new().build());

    let (block_requester, block_requests_rx, _justification_requests_rx) =
        TestBlockRequester::new();
    let (sender_tx, _sender_rx) = mpsc::unbounded();
    let (network_tx, network_rx) = mpsc::unbounded();
    let test_network = SimpleNetwork::new(network_rx, sender_tx);
    let data_store_config = DataStoreConfig {
        max_triggers_pending: 80_000,
        max_proposals_pending: 80_000,
        max_messages_pending: 40_000,
        available_proposals_cache_capacity: 8000,
        periodic_maintenance_interval: Duration::from_millis(20),
        request_block_after: Duration::from_millis(30),
    };

    // fn new<N: ComponentNetwork<Message, R = R>>(
    //         session_boundaries: SessionBoundaries<B>,
    //         client: Arc<C>,
    //         block_requester: RB,
    //         config: DataStoreConfig,
    //         component_network: N,
    //     )
    let session_period: SessionPeriod = SessionPeriod(50);
    let session_id = SessionId(0);
    let session_boundaries = SessionBoundaries::new(session_id, session_period);
    //DataStore<B, C, RB, Message, R>
    let (mut data_store, network) = DataStore::<
        Block,
        TestClient,
        TestBlockRequester<Block>,
        TestData,
        UnboundedReceiver<TestData>,
    >::new(
        session_boundaries,
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
        TestHandler {
            client,
            client_builder,
            block_requests_rx,
            network_tx,
            exit_data_store_tx,
            unique_seed: 0,
        },
        network,
    )
}

const TIMEOUT_SUCC: Duration = Duration::from_millis(5000);
const TIMEOUT_FAIL: Duration = Duration::from_millis(200);

// This is the basic assumption for other tests, so we better test it, in case this somehow changes in the future.
#[tokio::test]
async fn forks_have_different_block_hashes() {
    let (task_handle, mut test_handler, _network) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();
    let a1 = test_handler.build_block_at_hash(&genesis_hash).await;
    let b1 = test_handler.build_block_at_hash(&genesis_hash).await;
    assert!(a1.hash() != b1.hash());
}


#[tokio::test]
async fn correct_messages_go_through() {
    let (task_handle, mut test_handler, mut network) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, 20, false)
        .await;
    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());

        let message = timeout(TIMEOUT_SUCC, network.next())
            .await
            .ok()
            .flatten()
            .expect("Did not receive message from Data Store");
        assert_eq!(message.included_data(), test_data);
    }

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn too_long_branch_message_does_not_go_through() {
    let (task_handle, mut test_handler, mut network) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, 20, false)
        .await;

    let blocks_branch = blocks[0..((MAX_DATA_BRANCH_LEN+1) as usize)].to_vec();
    let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
    test_handler.send_data(test_data.clone());
    let res = timeout(TIMEOUT_FAIL, network.next()).await;
    assert!(res.is_err(), "Data Store let through a too long message");

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn branch_with_not_finalized_ancestor_correctly_handled() {
    let (task_handle, mut test_handler, mut network) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, 20, false)
        .await;

    let blocks_branch = blocks[1..2].to_vec();
    let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
    test_handler.send_data(test_data.clone());
    let res = timeout(TIMEOUT_FAIL, network.next()).await;
    assert!(res.is_err(), "Data Store let through a message with not finalized ancestor");

    // After the block gets finalized the message should be let through
    test_handler.finalize_block(&blocks[0].hash());
    let message = timeout(TIMEOUT_SUCC, network.next())
            .await
            .ok()
            .flatten()
            .expect("Did not receive message from Data Store");
    assert_eq!(message.included_data(), test_data);
    test_handler.exit();
    data_store_handle.await.unwrap();
}


#[tokio::test]
async fn correct_messages_go_through_with_late_import() {
    let (task_handle, mut test_handler, mut network) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_branch_upon(&genesis_hash, 20)
        .await;
    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());


    }
    let res = timeout(TIMEOUT_FAIL, network.next()).await;
    test_handler
        .import_branch(blocks, false)
        .await;

    for _ in 1..=MAX_DATA_BRANCH_LEN {
        let message = timeout(TIMEOUT_SUCC, network.next())
            .await
            .ok()
            .flatten()
            .expect("Did not receive message from Data Store");
    }


    test_handler.exit();
    data_store_handle.await.unwrap();
}

// #[tokio::test]
// async fn sends_messages_after_import() {
//     let (task_handle, mut test_handler, mut network) = prepare_data_store();
//     let data_store_handle = tokio::spawn(task_handle);

//     let block = test_handler.build_block();
//     let data = AlephData::from(block.clone());

//     test_handler.send_data(vec![data]);

//     test_handler.import_block(block, false).await;

//     let message = timeout(DEFAULT_TIMEOUT, network.next())
//         .await
//         .ok()
//         .flatten()
//         .expect("Did not receive message from Data Store");
//     assert_eq!(message.included_blocks(), vec![data]);

//     test_handler.exit();
//     data_store_handle.await.unwrap();
// }

// #[tokio::test]
// async fn sends_messages_with_number_lower_than_finalized() {
//     let (task_handle, mut test_handler, mut network) = prepare_data_store();
//     let data_store_handle = tokio::spawn(task_handle);

//     test_handler.build_and_import_blocks(4, true).await;

//     let mut blocks = Vec::new();
//     for i in 0u64..4 {
//         blocks.push(AlephData::from(
//             test_handler.build_block_at(i, i.to_le_bytes().to_vec()),
//         ));
//     }

//     test_handler.send_data(blocks.clone());

//     let message = timeout(DEFAULT_TIMEOUT, network.next())
//         .await
//         .ok()
//         .flatten()
//         .expect("Did not receive message from Data Store");
//     assert_eq!(message.included_blocks(), blocks);

//     test_handler.exit();
//     data_store_handle.await.unwrap();
// }

// #[tokio::test]
// async fn does_not_send_messages_without_import() {
//     let (task_handle, mut test_handler, mut network) = prepare_data_store();
//     let data_store_handle = tokio::spawn(task_handle);

//     let blocks = test_handler.build_and_import_blocks(4, true).await;

//     let not_imported_block = test_handler.build_block();

//     test_handler.send_data(vec![AlephData::from(not_imported_block)]);

//     test_handler.send_data(blocks.clone());

//     let message = timeout(DEFAULT_TIMEOUT, network.next())
//         .await
//         .ok()
//         .flatten()
//         .expect("Did not receive message from Data Store");
//     assert_eq!(message.included_blocks(), blocks);

//     test_handler.exit();

//     let message = network.next().await;
//     assert!(message.is_none());

//     data_store_handle.await.unwrap();
// }

// #[tokio::test]
// async fn sends_block_request_on_missing_block() {
//     let (task_handle, mut test_handler, _network) = prepare_data_store();
//     let data_store_handle = tokio::spawn(task_handle);

//     let data = AlephData::from(test_handler.build_block());

//     test_handler.send_data(vec![data]);

//     let requested_block = timeout(DEFAULT_TIMEOUT, test_handler.next_block_request())
//         .await
//         .expect("Did not receive block request from Data Store");
//     assert_eq!(requested_block, data);

//     test_handler.exit();
//     data_store_handle.await.unwrap();
// }
