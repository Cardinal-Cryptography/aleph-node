use parity_scale_codec::Codec;
use sc_consensus::import_queue::{ImportQueueService, IncomingBlock};
use sp_consensus::BlockOrigin;
use sp_runtime::{
    generic,
    traits::{CheckedSub, Header as _, Member, One},
};

use crate::{
    aleph_primitives::{Block, Header},
    block::{
        Block as BlockT, BlockId, BlockImport, Header as HeaderT, HeaderBackendStatus,
        UnverifiedHeader,
    },
    metrics::{AllBlockMetrics, Checkpoint},
    BlockHash,
};

type GenericBlock<E> = generic::Block<Header, E>;

mod chain_status;
mod finalizer;
mod justification;
mod status_notifier;
mod verification;

pub use chain_status::SubstrateChainStatus;
pub use justification::{
    InnerJustification, Justification, JustificationTranslator, TranslateError,
};
use primitives::BlockNumber;
pub use status_notifier::SubstrateChainStatusNotifier;
pub use verification::{SessionVerifier, SubstrateFinalizationInfo, VerifierCache};

use crate::block::{BlockchainEvents, HeaderBackend};

const LOG_TARGET: &str = "aleph-substrate";

impl UnverifiedHeader for Header {
    fn id(&self) -> BlockId {
        BlockId {
            hash: self.hash(),
            number: *self.number(),
        }
    }
}

impl HeaderT for Header {
    type Unverified = Self;

    fn id(&self) -> BlockId {
        BlockId {
            hash: self.hash(),
            number: *self.number(),
        }
    }

    fn parent_id(&self) -> Option<BlockId> {
        let number = self.number().checked_sub(&One::one())?;
        Some(BlockId {
            hash: *self.parent_hash(),
            number,
        })
    }

    fn into_unverified(self) -> Self::Unverified {
        self
    }
}

/// Wrapper around the trait object that we get from Substrate.
pub struct BlockImporter {
    importer: Box<dyn ImportQueueService<Block>>,
    metrics: AllBlockMetrics,
}

impl BlockImporter {
    pub fn new(importer: Box<dyn ImportQueueService<Block>>) -> Self {
        Self {
            importer,
            metrics: AllBlockMetrics::new(None),
        }
    }

    pub fn attach_metrics(&mut self, metrics: AllBlockMetrics) {
        self.metrics = metrics;
    }
}

impl BlockImport<Block> for BlockImporter {
    fn import_block(&mut self, block: Block, own: bool) {
        // We only need to distinguish between blocks produced by us and blocks incoming from the network
        // for the purpose of running `FinalityRateMetrics`. We use `BlockOrigin` to make this distinction.
        let origin = match own {
            true => BlockOrigin::Own,
            false => BlockOrigin::NetworkBroadcast,
        };
        let hash = block.header.hash();
        let number = *block.header.number();
        let incoming_block = IncomingBlock::<Block> {
            hash,
            header: Some(block.header),
            body: Some(block.extrinsics),
            indexed_body: None,
            justifications: None,
            origin: None,
            allow_missing_state: false,
            skip_execution: false,
            import_existing: false,
            state: None,
        };
        self.metrics
            .report_block(BlockId::new(hash, number), Checkpoint::Importing, Some(own));
        self.importer.import_blocks(origin, vec![incoming_block]);
    }
}

impl<E: Member + Codec> BlockT for GenericBlock<E> {
    type UnverifiedHeader = Header;

    /// The header of the block.
    fn header(&self) -> &Self::UnverifiedHeader {
        &self.header
    }
}

impl HeaderBackendStatus for sp_blockchain::Info<Block> {
    fn best_id(&self) -> BlockId {
        BlockId {
            hash: self.best_hash,
            number: self.best_number,
        }
    }

    fn genesis_hash(&self) -> BlockHash {
        self.genesis_hash
    }

    fn finalized_id(&self) -> BlockId {
        BlockId {
            hash: self.finalized_hash,
            number: self.finalized_number,
        }
    }
}

impl<HB: sp_blockchain::HeaderBackend<Block>> HeaderBackend<Header> for HB {
    type Status = sp_blockchain::Info<Block>;
    type Error = sp_blockchain::Error;

    fn header(&self, hash: BlockHash) -> Result<Option<Header>, sp_blockchain::Error> {
        self.header(hash)
    }

    fn hash(&self, number: BlockNumber) -> Result<Option<BlockHash>, Self::Error> {
        self.hash(number)
    }

    fn status(&self) -> Self::Status {
        self.info()
    }
}

impl<C: sc_client_api::BlockchainEvents<Block> + Send> BlockchainEvents<Header> for C {
    type ChainStatusNotifier = SubstrateChainStatusNotifier;

    fn chain_status_notifier(&self) -> SubstrateChainStatusNotifier {
        SubstrateChainStatusNotifier::new(
            self.finality_notification_stream(),
            self.every_import_notification_stream(),
        )
    }
}
