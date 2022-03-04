use crate::data_io::{
    AlephData, AlephNetworkMessage, DataStore, DataStoreConfig,
    UnvalidatedAlephProposal, MAX_DATA_BRANCH_LEN,
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
    runtime::{Block, Header},
    ClientBlockImportExt, ClientExt, DefaultTestClientBuilderExt, TestClient,
    TestClientBuilder, TestClientBuilderExt,
};

use tokio::time::timeout;


fn unvalidated_proposal_from_headers(blocks: Vec<Header>) -> UnvalidatedAlephProposal<Block> {
    let num = blocks.last().unwrap().number;
    let hashes = blocks.into_iter().map(|block| block.hash()).collect();
    UnvalidatedAlephProposal::new(hashes, num)
}

fn aleph_data_from_blocks(blocks: Vec<Block>) -> AlephData<Block> {
    let headers = blocks.into_iter().map(|b| b.header().clone()).collect();
    aleph_data_from_headers(headers)
}

fn aleph_data_from_headers(headers: Vec<Header>) -> AlephData<Block> {
    if headers.is_empty() {
        AlephData::Empty
    } else {
        AlephData::HeadProposal(unvalidated_proposal_from_headers(headers))
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
    network: Box<dyn DataNetwork<TestData>>,
    //<TestData , UnboundedReceiver<TestData>, UnboundedSender<TestData>>,
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
        self.client
            .finalize_block(BlockId::Hash(*hash), None)
            .unwrap();
    }

    fn genesis_hash(&mut self) -> H256 {
        assert_eq!(
            self.client.info().genesis_hash,
            self.client_builder.info().genesis_hash
        );
        self.client.info().genesis_hash
    }

    fn get_unique_bytes(&mut self) -> Vec<u8> {
        self.unique_seed += 1;
        self.unique_seed.to_be_bytes().to_vec()
    }

    fn get_header_at(&self, num: u64) -> Header {
        self.client_builder
            .header(&BlockId::Number(num))
            .unwrap()
            .unwrap()
    }

    async fn build_block_at_hash(&mut self, hash: &H256) -> Block {
        let unique_bytes: Vec<u8> = self.get_unique_bytes();
        let mut digest = Digest::default();
        digest.push(sp_runtime::generic::DigestItem::Other(unique_bytes));
        let block = self
            .client_builder
            .new_block_at(&BlockId::Hash(hash.clone()), digest, false)
            .unwrap()
            .build()
            .unwrap()
            .block;

        self.client_builder
            .import(BlockOrigin::Own, block.clone())
            .await
            .unwrap();
        block
    }

    /// Builds a sequence of blocks extending from `hash` of length `len`
    async fn build_branch_upon(&mut self, hash: &H256, len: usize) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut prev_hash = *hash;
        for _i in 0..len {
            let block = self.build_block_at_hash(&prev_hash).await;
            prev_hash = block.hash();
            blocks.push(block);
        }
        blocks
    }

    /// imports a sequence of blocks, should be in correct order
    async fn import_branch(&mut self, blocks: Vec<Block>, finalize: bool) {
        for block in blocks {
            self.import_block(block.clone(), finalize).await;
        }
    }

    /// Builds a sequence of blocks extending from `hash` of length `len` and imports them
    async fn build_and_import_branch_upon(
        &mut self,
        hash: &H256,
        len: usize,
        finalize: bool,
    ) -> Vec<Block> {
        let blocks = self.build_branch_upon(hash, len).await;
        self.import_branch(blocks.clone(), finalize).await;
        blocks
    }

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

    async fn assert_no_message_out(&mut self, err_message: &'static str) {
        assert!(
            timeout(TIMEOUT_FAIL, self.network.next()).await.is_err(),
            "{}", err_message
        );
    }

    async fn assert_message_out(&mut self, err_message: &'static str) -> TestData {

        timeout(TIMEOUT_SUCC, self.network.next())
            .await
            .expect(err_message)
            .expect(err_message)
    }

}




fn prepare_data_store() -> (
    impl Future<Output = ()>,
    TestHandler,
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
            network: Box::new(network),
        }
    )
}

const TIMEOUT_SUCC: Duration = Duration::from_millis(5000);
const TIMEOUT_FAIL: Duration = Duration::from_millis(200);

// This is the basic assumption for other tests, so we better test it, in case this somehow changes in the future.
#[tokio::test]
async fn forks_have_different_block_hashes() {
    let (_task_handle, mut test_handler) = prepare_data_store();
    let genesis_hash = test_handler.genesis_hash();
    let a1 = test_handler.build_block_at_hash(&genesis_hash).await;
    let b1 = test_handler.build_block_at_hash(&genesis_hash).await;
    assert!(a1.hash() != b1.hash());
}

#[tokio::test]
async fn correct_messages_go_through() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10, false)
        .await;
    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());

        let message = test_handler.assert_message_out("Did not receive message from Data Store").await;
        assert_eq!(message.included_data(), test_data);
    }

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn too_long_branch_message_does_not_go_through() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10, false)
        .await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN + 2].hash());

    let blocks_branch = blocks[0..((MAX_DATA_BRANCH_LEN + 1) as usize)].to_vec();
    let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
    test_handler.send_data(test_data.clone());
    test_handler.assert_no_message_out("Data Store let through a too long message").await;

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn branch_with_not_finalized_ancestor_correctly_handled() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10, false)
        .await;

    let blocks_branch = blocks[1..2].to_vec();
    let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
    test_handler.send_data(test_data.clone());

    test_handler.assert_no_message_out("Data Store let through a message with not finalized ancestor").await;

    // After the block gets finalized the message should be let through
    test_handler.finalize_block(&blocks[0].hash());
    let message = test_handler.assert_message_out("Did not receive message from Data Store").await;
    assert_eq!(message.included_data(), test_data);
    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn correct_messages_go_through_with_late_import() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10)
        .await;
    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());
    }


    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.import_branch(blocks, false).await;

    for _ in 1..=MAX_DATA_BRANCH_LEN {
        let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;
    }

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn message_with_multiple_data_gets_through_when_it_should() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10, false)
        .await;
    let mut test_data = vec![];
    for i in 1..(MAX_DATA_BRANCH_LEN + 10) {
        let blocks_branch = blocks[i..(i + 1)].to_vec();
        test_data.push(aleph_data_from_blocks(blocks_branch));
    }
    test_handler.send_data(test_data.clone());

    test_handler.assert_no_message_out("Data Store let through a message with not finalized ancestor").await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN + 7].hash());
    // This should not be enough yet (ancestor not finalized for some data items)

    test_handler.assert_no_message_out("Data Store let through a message with not finalized ancestor").await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN + 8].hash());

    let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;
    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn sends_block_request_on_missing_block() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10)
        .await;
    let blocks_branch = blocks[0..1].to_vec();
    let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
    test_handler.send_data(test_data.clone());

    test_handler.assert_no_message_out("Data Store let through a message with not finalized ancestor").await;

    let requested_block = timeout(TIMEOUT_SUCC, test_handler.next_block_request())
        .await
        .expect("Did not receive block request from Data Store");
    assert_eq!(requested_block.hash, blocks[0].hash());

    test_handler.import_branch(blocks, false).await;

    let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn does_not_send_requests_when_no_block_missing() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10, false)
        .await;

    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());
    }
    assert!(
        timeout(TIMEOUT_FAIL, test_handler.next_block_request()).await.is_err(),
        "Data Store is sending block requests without reason"
    );

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn message_with_genesis_block_does_not_get_through() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let _blocks = test_handler
        .build_and_import_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10, false)
        .await;

    let test_data: TestData = vec![aleph_data_from_headers(vec![test_handler.get_header_at(0)])];
    test_handler.send_data(test_data.clone());

    test_handler.assert_no_message_out("Data Store let through a message with genesis block proposal").await;

    test_handler.exit();
    data_store_handle.await.unwrap();
}



#[tokio::test]
async fn unimported_hopeless_forks_go_through() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10)
        .await;
    let forking_block = &blocks[MAX_DATA_BRANCH_LEN+2];
    let forks = test_handler
        .build_branch_upon(&forking_block.hash(), MAX_DATA_BRANCH_LEN*10)
        .await;

    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = forks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());
    }

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.import_branch(blocks.clone(), false).await;

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN+1].hash());

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN*2+1].hash());

    for _ in 1..=MAX_DATA_BRANCH_LEN {
        let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;
    }

    test_handler.exit();
    data_store_handle.await.unwrap();
}


#[tokio::test]
async fn imported_hopeless_forks_go_through() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10)
        .await;
    let forking_block = &blocks[MAX_DATA_BRANCH_LEN+2];
    let forks = test_handler
        .build_branch_upon(&forking_block.hash(), MAX_DATA_BRANCH_LEN*10)
        .await;

    test_handler.import_branch(blocks.clone(), false).await;
    test_handler.import_branch(forks.clone(), false).await;

    for i in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = forks[0..(i as usize)].to_vec();
        let test_data: TestData = vec![aleph_data_from_blocks(blocks_branch)];
        test_handler.send_data(test_data.clone());
    }

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN+1].hash());

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.finalize_block(&blocks[MAX_DATA_BRANCH_LEN*2+1].hash());

    for _ in 1..=MAX_DATA_BRANCH_LEN {
        let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;
    }

    test_handler.exit();
    data_store_handle.await.unwrap();
}

#[tokio::test]
async fn hopeless_fork_at_the_boundary_goes_through() {
    let (task_handle, mut test_handler) = prepare_data_store();
    let data_store_handle = tokio::spawn(task_handle);
    let genesis_hash = test_handler.genesis_hash();

    let blocks = test_handler
        .build_branch_upon(&genesis_hash, MAX_DATA_BRANCH_LEN * 10)
        .await;
    let fork_num = MAX_DATA_BRANCH_LEN+2;
    let forking_block = &blocks[fork_num];
    let forks = test_handler
        .build_branch_upon(&forking_block.hash(), MAX_DATA_BRANCH_LEN*10)
        .await;

    test_handler.import_branch(blocks.clone(), false).await;

    // the latter skips one block
    let honest_hopeless_fork = vec![blocks[fork_num-2].clone(), blocks[fork_num-1].clone(), blocks[fork_num].clone(), forks[0].clone()];
    let honest_hopeless_fork2 = vec![blocks[fork_num-2].clone(), blocks[fork_num-1].clone(), blocks[fork_num].clone(), forks[0].clone(), forks[1].clone()];
    let malicious_hopeless_fork = vec![blocks[fork_num-2].clone(), blocks[fork_num-1].clone(), blocks[fork_num].clone(), forks[1].clone()];
    let malicious_hopeless_fork2 = vec![blocks[fork_num-2].clone(), blocks[fork_num-1].clone(), blocks[fork_num].clone(),blocks[fork_num+1].clone(), forks[1].clone()];
    let honest_data = vec![aleph_data_from_blocks(honest_hopeless_fork )];
    let honest_data2 = vec![aleph_data_from_blocks(honest_hopeless_fork2 )];
    let malicious_data = vec![aleph_data_from_blocks(malicious_hopeless_fork )];
    let malicious_data2 = vec![aleph_data_from_blocks(malicious_hopeless_fork2 )];

    test_handler.send_data(honest_data);
    test_handler.send_data(honest_data2);
    test_handler.send_data(malicious_data);
    test_handler.send_data(malicious_data2);

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.finalize_block(&blocks[fork_num].hash());

    // the malicious_data should come here

    let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

    test_handler.finalize_block(&blocks[fork_num+1].hash());


    let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;

    let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;

    test_handler.assert_no_message_out("Data Store let through a message with not yet imported blocks").await;

        test_handler.finalize_block(&blocks[fork_num+2].hash());

    let _message = test_handler.assert_message_out("Did not receive message from Data Store").await;

    test_handler.exit();
    data_store_handle.await.unwrap();
}

