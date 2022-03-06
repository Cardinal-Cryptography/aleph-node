use crate::BlockHashNum;

use sc_block_builder::BlockBuilderProvider;
use sc_client_api::HeaderBackend;
use sp_api::BlockId;
use sp_consensus::BlockOrigin;
pub use sp_core::hash::H256;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::Digest;
use std::{default::Default, sync::Arc};
use substrate_test_runtime_client::{
    runtime::{Block, Header},
    ClientBlockImportExt, ClientExt, TestClient,
};

// A helper struct that allows to build blocks without importing/finalizing them right away.
pub struct ChainBuilder {
    pub client: Arc<TestClient>,
    pub client_builder: Arc<TestClient>,
    pub unique_seed: u32,
}

impl ChainBuilder {
    pub fn new(client: Arc<TestClient>, client_builder: Arc<TestClient>) -> Self {
        ChainBuilder {
            client,
            client_builder,
            unique_seed: 0,
        }
    }

    /// Import block in test client
    pub async fn import_block(&mut self, block: Block, finalize: bool) {
        if finalize {
            self.client.import_as_final(BlockOrigin::Own, block.clone())
        } else {
            self.client.import(BlockOrigin::Own, block.clone())
        }
        .await
        .unwrap();
    }

    /// Finalize block with given hash without providing justification.
    pub fn finalize_block(&self, hash: &H256) {
        self.client
            .finalize_block(BlockId::Hash(*hash), None)
            .unwrap();
    }

    pub fn genesis_hash_num(&self) -> BlockHashNum<Block> {
        assert_eq!(
            self.client.info().genesis_hash,
            self.client_builder.info().genesis_hash
        );
        BlockHashNum::<Block>::new(self.client.info().genesis_hash, 0u64)
    }

    pub fn genesis_hash(&self) -> H256 {
        self.genesis_hash_num().hash
    }

    pub fn get_unique_bytes(&mut self) -> Vec<u8> {
        self.unique_seed += 1;
        self.unique_seed.to_be_bytes().to_vec()
    }

    pub async fn build_block_at_hash(&mut self, hash: &H256) -> Block {
        let unique_bytes: Vec<u8> = self.get_unique_bytes();
        let mut digest = Digest::default();
        digest.push(sp_runtime::generic::DigestItem::Other(unique_bytes));
        let block = self
            .client_builder
            .new_block_at(&BlockId::Hash(*hash), digest, false)
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
    pub async fn build_branch_upon(&mut self, hash: &H256, len: usize) -> Vec<Block> {
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
    pub async fn import_branch(&mut self, blocks: Vec<Block>, finalize: bool) {
        for block in blocks {
            self.import_block(block.clone(), finalize).await;
        }
    }

    /// Builds a sequence of blocks extending from `hash` of length `len` and imports them
    pub async fn build_and_import_branch_upon(
        &mut self,
        hash: &H256,
        len: usize,
        finalize: bool,
    ) -> Vec<Block> {
        let blocks = self.build_branch_upon(hash, len).await;
        self.import_branch(blocks.clone(), finalize).await;
        blocks
    }

    pub fn get_header_at(&self, num: u64) -> Header {
        self.client_builder
            .header(&BlockId::Number(num))
            .unwrap()
            .unwrap()
    }
}
