use std::sync::Arc;

use sp_api::BlockId;
use sp_blockchain::{
    Backend as BlockchainBackend, BlockStatus, HeaderBackend, HeaderMetadata, Info,
};
use sp_runtime::traits::Block;

use crate::{
    testing::mocks::{TBlock, THash, THeader, TNumber},
    GetBlockchainBackend,
};

#[derive(Clone)]
pub(crate) struct Backend {
    blocks: Vec<TBlock>,
    next_block_to_finalize: TBlock,
}

pub(crate) fn create_block(parent_hash: THash, number: TNumber) -> TBlock {
    TBlock {
        header: THeader {
            parent_hash,
            number,
            state_root: Default::default(),
            extrinsics_root: Default::default(),
            digest: Default::default(),
        },
        extrinsics: vec![],
    }
}

const GENESIS_HASH: [u8; 32] = [0u8; 32];

impl Backend {
    pub(crate) fn new(finalized_height: u64) -> Self {
        let mut blocks: Vec<TBlock> = vec![];

        for n in 1u64..=finalized_height {
            let parent_hash = match n {
                1 => GENESIS_HASH.into(),
                _ => blocks.last().unwrap().header.hash(),
            };
            blocks.push(create_block(parent_hash, n));
        }

        let next_block_to_finalize =
            create_block(blocks.last().unwrap().hash(), finalized_height + 1);

        Backend {
            blocks,
            next_block_to_finalize,
        }
    }

    pub(crate) fn next_block_to_finalize(&self) -> TBlock {
        self.next_block_to_finalize.clone()
    }

    pub(crate) fn get_block(&self, id: BlockId<TBlock>) -> Option<TBlock> {
        match id {
            BlockId::Hash(h) => {
                if self.next_block_to_finalize.hash() == h {
                    Some(self.next_block_to_finalize.clone())
                } else {
                    self.blocks.iter().find(|b| b.header.hash().eq(&h)).cloned()
                }
            }
            BlockId::Number(n) => {
                if self.next_block_to_finalize.header.number == n {
                    Some(self.next_block_to_finalize.clone())
                } else {
                    self.blocks.get((n - 1) as usize).cloned()
                }
            }
        }
    }
}

impl HeaderBackend<TBlock> for Backend {
    fn header(&self, id: BlockId<TBlock>) -> sp_blockchain::Result<Option<THeader>> {
        Ok(self.get_block(id).map(|b| b.header))
    }

    fn info(&self) -> Info<TBlock> {
        Info {
            best_hash: self.next_block_to_finalize.hash(),
            best_number: self.next_block_to_finalize.header.number,
            finalized_hash: self.blocks.last().unwrap().hash(),
            finalized_number: self.blocks.len() as u64,
            genesis_hash: GENESIS_HASH.into(),
            number_leaves: Default::default(),
            finalized_state: None,
            block_gap: None,
        }
    }

    fn status(&self, id: BlockId<TBlock>) -> sp_blockchain::Result<BlockStatus> {
        Ok(match self.get_block(id) {
            Some(_) => BlockStatus::InChain,
            _ => BlockStatus::Unknown,
        })
    }

    fn number(&self, hash: THash) -> sp_blockchain::Result<Option<TNumber>> {
        Ok(self.get_block(BlockId::hash(hash)).map(|b| b.header.number))
    }

    fn hash(&self, number: TNumber) -> sp_blockchain::Result<Option<THash>> {
        Ok(self.get_block(BlockId::Number(number)).map(|b| b.hash()))
    }
}

impl HeaderMetadata<TBlock> for Backend {
    type Error = sp_blockchain::Error;
    fn header_metadata(
        &self,
        _hash: <TBlock as Block>::Hash,
    ) -> Result<sp_blockchain::CachedHeaderMetadata<TBlock>, Self::Error> {
        Err(sp_blockchain::Error::Backend(
            "Header metadata not implemented".into(),
        ))
    }
    fn insert_header_metadata(
        &self,
        _hash: <TBlock as Block>::Hash,
        _header_metadata: sp_blockchain::CachedHeaderMetadata<TBlock>,
    ) {
    }
    fn remove_header_metadata(&self, _hash: <TBlock as Block>::Hash) {}
}

impl BlockchainBackend<TBlock> for Backend {
    fn body(
        &self,
        _id: BlockId<TBlock>,
    ) -> sp_blockchain::Result<Option<Vec<<TBlock as Block>::Extrinsic>>> {
        Ok(None)
    }
    fn justifications(
        &self,
        _id: BlockId<TBlock>,
    ) -> sp_blockchain::Result<Option<sp_runtime::Justifications>> {
        Ok(None)
    }
    fn last_finalized(&self) -> sp_blockchain::Result<<TBlock as Block>::Hash> {
        Ok(self.info().finalized_hash)
    }
    fn leaves(&self) -> sp_blockchain::Result<Vec<<TBlock as Block>::Hash>> {
        Ok(vec![self.next_block_to_finalize.hash()])
    }
    fn displaced_leaves_after_finalizing(
        &self,
        _block_number: sp_api::NumberFor<TBlock>,
    ) -> sp_blockchain::Result<Vec<<TBlock as Block>::Hash>> {
        Ok(Vec::new())
    }
    fn children(&self, parent_hash: THash) -> sp_blockchain::Result<Vec<THash>> {
        let leaves = if self.next_block_to_finalize.hash() == parent_hash {
            Vec::new()
        } else if self
            .blocks
            .last()
            .map(|b| b.hash())
            .unwrap()
            .eq(&parent_hash)
        {
            vec![self.next_block_to_finalize.hash()]
        } else {
            self.blocks
                .windows(2)
                .flat_map(<&[TBlock; 2]>::try_from)
                .find(|[parent, _]| parent.header.hash().eq(&parent_hash))
                .map(|[_, c]| vec![c.hash()])
                .unwrap_or_default()
        };
        Ok(leaves)
    }
    fn indexed_transaction(&self, _hash: &THash) -> sp_blockchain::Result<Option<Vec<u8>>> {
        Ok(None)
    }
    fn block_indexed_body(
        &self,
        _id: BlockId<TBlock>,
    ) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
        Ok(None)
    }
}

unsafe impl Send for Backend {}

unsafe impl Sync for Backend {}

pub(crate) struct GetBlockchainBackendMock {
    pub(crate) backend: Arc<Backend>,
}

impl GetBlockchainBackend<TBlock> for GetBlockchainBackendMock {
    type BlockchainBackend = Backend;
    fn blockchain(&self) -> &Self::BlockchainBackend {
        &self.backend
    }
}
