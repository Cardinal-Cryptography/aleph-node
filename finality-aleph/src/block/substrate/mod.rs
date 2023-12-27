use sc_consensus::import_queue::{ImportQueueService, IncomingBlock};
use sp_consensus::BlockOrigin;
use sp_runtime::traits::{CheckedSub, Header as _, One};

use crate::{
    aleph_primitives::{Block, Header},
    block::{Block as BlockT, BlockId, BlockImport, Header as HeaderT, UnverifiedHeader},
    metrics::{AllBlockMetrics, Checkpoint},
    BlockHash,
};

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

use crate::block::{BlockchainEvents, HeaderBackend, SelectChain, SelectChainError};

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

impl BlockT for Block {
    type UnverifiedHeader = Header;

    /// The header of the block.
    fn header(&self) -> &Self::UnverifiedHeader {
        &self.header
    }
}

#[derive(Debug)]
pub enum HeaderBackendError {
    NotFinalized(BlockNumber),
    UnknownHeader(BlockId),
    NoHashForNumber(BlockNumber),
}

impl<HB: sp_blockchain::HeaderBackend<Block>> HeaderBackend<Header> for HB {
    type Error = HeaderBackendError;

    fn header(&self, id: BlockId) -> Result<Option<Header>, Self::Error> {
        self.header(id.hash)
            .map_err(|_| Self::Error::UnknownHeader(id))
    }

    fn finalized_hash(&self, number: BlockNumber) -> Result<BlockHash, Self::Error> {
        if self.top_finalized().number() < number {
            return Err(Self::Error::NotFinalized(number));
        }

        return match self.hash(number).ok().flatten() {
            None => {
                log::error!(target: "chain-info", "Could not get hash for block #{:?}", number);
                Err(Self::Error::NoHashForNumber(number))
            }
            Some(h) => Ok(h),
        };
    }

    fn top_finalized(&self) -> BlockId {
        let info = self.info();
        BlockId {
            hash: info.finalized_hash,
            number: info.finalized_number,
        }
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

#[async_trait::async_trait]
impl<SC: sp_consensus::SelectChain<Block>> SelectChain<Header> for SC {
    async fn leaves(&self) -> Result<Vec<BlockHash>, SelectChainError> {
        self.leaves().await
    }

    async fn best_chain(&self) -> Result<Header, SelectChainError> {
        self.best_chain().await
    }

    async fn finality_target(
        &self,
        base_hash: BlockHash,
        maybe_max_number: Option<BlockNumber>,
    ) -> Result<BlockHash, SelectChainError> {
        self.finality_target(base_hash, maybe_max_number).await
    }
}
