use super::*;
use codec::{Decode, Encode};
use rush::{nodes::NodeIndex, PreUnit};
use sp_consensus_aura::sr25519::AuthorityId;
use sp_keystore::CryptoStore;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Hash as _Hash};
use std::sync::Mutex;

#[derive(Debug, PartialEq, Clone, Encode, Decode, Eq, serde::Serialize)]
struct Extrinsic {}

parity_util_mem::malloc_size_of_is_0!(Extrinsic);

impl sp_runtime::traits::Extrinsic for Extrinsic {
    type Call = Extrinsic;
    type SignaturePayload = ();
}

mod dummy;

type BlockNumber = u64;
type Hashing = BlakeTwo256;
type Header = sp_runtime::generic::Header<BlockNumber, Hashing>;
type Hash = sp_core::H256;
type Block = sp_runtime::generic::Block<Header, Extrinsic>;
type Backend = sc_client_api::in_mem::Backend<Block>;

fn make_chain(n: u64) -> Vec<Block> {
    let block0 = Block {
        header: Header {
            parent_hash: Default::default(),
            number: 0,
            state_root: Default::default(),
            extrinsics_root: Default::default(),
            digest: Default::default(),
        },
        extrinsics: vec![],
    };
    let mut blocks = vec![block0];
    for i in 0..n - 1 {
        let block = Block {
            header: Header {
                parent_hash: blocks[i as usize].hash(),
                number: i + 1,
                state_root: Default::default(),
                extrinsics_root: Default::default(),
                digest: Default::default(),
            },
            extrinsics: vec![],
        };
        blocks.push(block);
    }
    blocks
}
#[derive(Clone)]
struct SelectChain {
    block: Arc<Mutex<Option<Block>>>,
}

impl SelectChain {
    fn new() -> Self {
        SelectChain {
            block: Arc::new(Mutex::new(None)),
        }
    }
    fn set_block(&self, block: Block) {
        *self.block.lock().unwrap() = Some(block);
    }
}

impl sp_consensus::SelectChain<Block> for SelectChain {
    fn leaves(&self) -> Result<Vec<Hash>, sp_consensus::Error> {
        let block = self.block.lock().unwrap().as_ref().unwrap().clone();
        let hash = BlockT::hash(&block);
        Ok(vec![hash])
    }

    fn best_chain(&self) -> Result<Header, sp_consensus::Error> {
        Ok(self.block.lock().unwrap().clone().unwrap().header)
    }
}

use crate::KEY_TYPE;

async fn generate_authority_keystore(s: &str) -> AuthorityKeystore {
    let keystore = Arc::new(sp_keystore::testing::KeyStore::new());
    let pk = keystore
        .sr25519_generate_new(KEY_TYPE, Some(s))
        .await
        .unwrap();
    assert_eq!(keystore.keys(KEY_TYPE).await.unwrap().len(), 3);
    let authority_id = AuthorityId::from(pk);
    AuthorityKeystore::new(authority_id, keystore)
}

/// A simple scenario with three nodes: Alice, Bob and Charlie. We create an Environment for Alice
/// and simulate the behavior of Bob and Charlie by passinng appropriate messages to the sinks
/// from the environment. Three blocks are proposed in the same order by all nodes.
#[test]
fn test_simple_scenario() {
    // Channels creation
    let (notification_in_tx, mut notification_in_rx) = futures::channel::mpsc::unbounded();
    let (mut notification_out_tx, notification_out_rx) = futures::channel::mpsc::unbounded();
    let (network_command_tx, mut network_command_rx) = futures::channel::mpsc::unbounded();
    let (mut network_event_tx, network_event_rx) = futures::channel::mpsc::unbounded();
    let (_order_tx, order_rx) = tokio::sync::mpsc::unbounded_channel();

    // tokio runtime
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    // generate the keys and indices for the nodes
    let alice_authority_keystore = rt.block_on(generate_authority_keystore("//Alice"));
    let bob_authority_keystore = rt.block_on(generate_authority_keystore("//Bob"));
    let charlie_authority_keystore = rt.block_on(generate_authority_keystore("//Charlie"));

    let alice_node_index: NodeIndex = 0.into();
    let bob_node_index: NodeIndex = 1.into();
    let charlie_node_index: NodeIndex = 2.into();

    let _alice_peer_id = PeerId::random();
    let bob_peer_id = PeerId::random();
    let charlie_peer_id = PeerId::random();

    let n_rounds = 3_u64;

    let blocks = make_chain(n_rounds);

    // Create environment
    let client: Arc<dummy::Dummy<Backend>> = Arc::new(Default::default());
    let select_chain = SelectChain::new();
    let env = Environment::new(
        client,
        select_chain.clone(),
        notification_in_tx,
        notification_out_rx,
        network_command_tx,
        network_event_rx,
        order_rx,
        alice_authority_keystore,
        |data| <BlakeTwo256 as sp_core::Hasher>::hash(data),
        EpochId(0),
    );

    // spawn a task simulating the consensus
    let consensus_handle = {
        let blocks = blocks.clone();
        rt.spawn(async move {
            let mut prev_hash = None;
            let mut all_units = vec![];
            for (round, block) in blocks.iter().enumerate() {
                select_chain.set_block(block.clone());
                let node_map = vec![prev_hash; 3].into();
                let pre_unit =
                    PreUnit::new_from_parents(alice_node_index, round, node_map, BlakeTwo256::hash);
                prev_hash = Some(block.hash());
                notification_out_tx
                    .send(NotificationOut::CreatedPreUnit(pre_unit))
                    .await
                    .unwrap();
                let mut did_create = [false; 3];
                for _i in 0..3_usize {
                    let units = match notification_in_rx.next().await {
                        Some(NotificationIn::NewUnits(units)) => units,
                        _ => panic!("New units expected"),
                    };
                    all_units.extend_from_slice(&units);
                    assert_eq!(units.len(), 1);
                    let creator = units[0].creator();
                    did_create[creator.0] = true;
                    assert_eq!(units[0].round(), round);
                }
                assert_eq!(did_create, [true; 3]);
            }
            // For simplicity, we do not emit the batch of blocks. At the moment of writing tests,
            // Environment's run_epoch method does not have a nice termination condition,
            // and anyway implementing client's logic would be too troublesome.
            // order_tx.send(all_units.iter().map(|u| u.hash()).collect()).unwrap();
        })
    };

    // spawn the network task
    let network_handle = rt.spawn(async move {
        // not sure this is needed
        network_event_tx
            .send(NetworkEvent::PeerConnected(bob_peer_id))
            .await
            .unwrap();
        network_event_tx
            .send(NetworkEvent::PeerConnected(charlie_peer_id))
            .await
            .unwrap();
        for round in 0..n_rounds {
            let signed_unit = match network_command_rx.next().await {
                Some(NetworkCommand::SendToAll(NetworkMessage::Consensus(
                    ConsensusMessage::NewUnit(su),
                    EpochId(0),
                ))) => su,
                _ => panic!("Expected send new unit to all"),
            };
            assert!(signed_unit.verify_unit_signature());
            let pre_unit = signed_unit.unit.inner;
            assert_eq!(pre_unit.round(), round as usize);
            assert_eq!(pre_unit.creator(), alice_node_index);

            let bob_and_charlie = [
                (bob_node_index, bob_authority_keystore.clone(), bob_peer_id),
                (
                    charlie_node_index,
                    charlie_authority_keystore.clone(),
                    charlie_peer_id,
                ),
            ];

            for (node_index, keystore, peer_id) in bob_and_charlie.iter() {
                let pre_unit = PreUnit::new_from_parents(
                    *node_index,
                    round as usize,
                    vec![None; 3].into(),
                    BlakeTwo256::hash,
                );
                let full_unit = FullUnit {
                    inner: pre_unit,
                    block_hash: blocks[round as usize].hash(),
                };
                let signed_unit = sign_unit(&keystore, full_unit);

                network_event_tx
                    .send(NetworkEvent::MessageReceived(
                        ConsensusMessage::NewUnit(signed_unit),
                        *peer_id,
                    ))
                    .await
                    .unwrap();
            }
        }
    });

    rt.block_on(env.run_epoch());

    rt.block_on(consensus_handle).unwrap();
    rt.block_on(network_handle).unwrap();
}
