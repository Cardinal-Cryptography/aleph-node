use aleph_primitives::BlockNumber;
use sp_api::BlockId;
use sp_blockchain::Info;
use sp_runtime::traits::Block;

use crate::{
    testing::mocks::{TBlock, TBlockIdentifier, THash, THeader},
    BlockIdentifier, BlockchainBackend, ChainInfo,
};

#[derive(Clone)]
pub struct Backend {
    blocks: Vec<TBlockIdentifier>,
    next_block_to_finalize: TBlockIdentifier,
}

pub fn create_block(parent_hash: THash, number: BlockNumber) -> TBlockIdentifier {
    TBlockIdentifier {
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
    pub fn new(finalized_height: BlockNumber) -> Self {
        let mut blocks: Vec<TBlock> = vec![];

        for n in 1..=finalized_height {
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

    pub fn next_block_to_finalize(&self) -> TBlock {
        self.next_block_to_finalize.clone()
    }

    pub fn get_block(&self, id: BlockId<TBlock>) -> Option<TBlock> {
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

impl BlockchainBackend<TBlockIdentifier> for Backend {
    fn children(&self, parent_id: TBlockIdentifier) -> Vec<THash> {
        if self.next_block_to_finalize == parent_id {
            Vec::new()
        } else if self.blocks.last().map(|b| b.hash()).unwrap().eq(&parent_id) {
            vec![self.next_block_to_finalize.hash()]
        } else {
            self.blocks
                .windows(2)
                .flat_map(<&[TBlock; 2]>::try_from)
                .find(|[parent, _]| parent.header.hash().eq(&parent_id.block_hash()))
                .map(|[_, c]| vec![c.hash()])
                .unwrap_or_default()
        }
    }
    fn id(&self, block_number: BlockNumber) -> sp_blockchain::Result<Option<TBlockIdentifier>> {
        Ok(
            if self.next_block_to_finalize.header.number == block_number {
                Some(self.next_block_to_finalize.clone())
            } else {
                self.blocks.get((block_number - 1) as usize).cloned()
            },
        )
    }

    fn info(&self) -> ChainInfo<TBlockIdentifier> {
        ChainInfo {
            best_block: self.next_block_to_finalize,
            finalized_block: self.blocks.last().unwrap().clone(),
        }
    }
}

unsafe impl Send for Backend {}

unsafe impl Sync for Backend {}
