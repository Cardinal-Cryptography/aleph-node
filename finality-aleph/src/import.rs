use std::{fmt::Debug, time::Instant};

use futures::channel::mpsc::{TrySendError, UnboundedSender};
use log::{debug, warn};
use sc_consensus::{
    BlockCheckParams, BlockImport, BlockImportParams, ImportResult, JustificationImport,
};
use sp_consensus::Error as ConsensusError;
use sp_runtime::{traits::Header as HeaderT, Justification as SubstrateJustification};

use crate::{
    aleph_primitives::{Block, BlockHash, BlockNumber, Header, ALEPH_ENGINE_ID},
    justification::{backwards_compatible_decode, DecodeError},
    metrics::{Checkpoint, Metrics},
    sync::substrate::{Justification, JustificationTranslator},
    BlockId,
};

/// A wrapper around a block import that also marks the start and end of the import of every block
/// in the metrics, if provided.
#[derive(Clone)]
pub struct TracingBlockImport<I>
where
    I: BlockImport<Block> + Send + Sync,
{
    inner: I,
    metrics: Metrics<BlockHash>,
}

impl<I> TracingBlockImport<I>
where
    I: BlockImport<Block> + Send + Sync,
{
    pub fn new(inner: I, metrics: Metrics<BlockHash>) -> Self {
        TracingBlockImport { inner, metrics }
    }
}
#[async_trait::async_trait]
impl<I> BlockImport<Block> for TracingBlockImport<I>
where
    I: BlockImport<Block> + Send + Sync,
{
    type Error = I::Error;
    type Transaction = I::Transaction;

    async fn check_block(
        &mut self,
        block: BlockCheckParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        self.inner.check_block(block).await
    }

    async fn import_block(
        &mut self,
        block: BlockImportParams<Block, Self::Transaction>,
    ) -> Result<ImportResult, Self::Error> {
        let post_hash = block.post_hash();
        self.metrics
            .report_block(post_hash, Instant::now(), Checkpoint::Importing);

        let result = self.inner.import_block(block).await;

        if let Ok(ImportResult::Imported(_)) = &result {
            self.metrics
                .report_block(post_hash, Instant::now(), Checkpoint::Imported);
        }
        result
    }
}

/// A wrapper around a block import that also extracts any present justifications and sends them to
/// our components which will process them further and possibly finalize the block.
#[derive(Clone)]
pub struct AlephBlockImport<I, JT>
where
    I: BlockImport<Block> + Clone + Send,
    JT: JustificationTranslator,
{
    inner: I,
    justification_tx: UnboundedSender<Justification>,
    translator: JT,
}

#[derive(Debug)]
enum SendJustificationError<TE: Debug> {
    Send(Box<TrySendError<Justification>>),
    Consensus(Box<ConsensusError>),
    Decode(DecodeError),
    Translate(TE),
}

impl<TE: Debug> From<DecodeError> for SendJustificationError<TE> {
    fn from(decode_error: DecodeError) -> Self {
        Self::Decode(decode_error)
    }
}

impl<I, JT> AlephBlockImport<I, JT>
where
    I: BlockImport<Block> + Clone + Send,
    JT: JustificationTranslator,
{
    pub fn new(
        inner: I,
        justification_tx: UnboundedSender<Justification>,
        translator: JT,
    ) -> AlephBlockImport<I, JT> {
        AlephBlockImport {
            inner,
            justification_tx,
            translator,
        }
    }

    fn send_justification(
        &mut self,
        block_id: BlockId<Header>,
        justification: SubstrateJustification,
    ) -> Result<(), SendJustificationError<JT::Error>> {
        debug!(target: "aleph-justification", "Importing justification for block {}.", block_id);
        if justification.0 != ALEPH_ENGINE_ID {
            return Err(SendJustificationError::Consensus(Box::new(
                ConsensusError::ClientImport("Aleph can import only Aleph justifications.".into()),
            )));
        }
        let justification_raw = justification.1;
        let aleph_justification = backwards_compatible_decode(justification_raw)?;
        let justification = self
            .translator
            .translate(aleph_justification, block_id)
            .map_err(SendJustificationError::Translate)?;

        self.justification_tx
            .unbounded_send(justification)
            .map_err(|e| SendJustificationError::Send(Box::new(e)))
    }
}

#[async_trait::async_trait]
impl<I, JT> BlockImport<Block> for AlephBlockImport<I, JT>
where
    I: BlockImport<Block> + Clone + Send,
    JT: JustificationTranslator,
{
    type Error = I::Error;
    type Transaction = I::Transaction;

    async fn check_block(
        &mut self,
        block: BlockCheckParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        self.inner.check_block(block).await
    }

    async fn import_block(
        &mut self,
        mut block: BlockImportParams<Block, Self::Transaction>,
    ) -> Result<ImportResult, Self::Error> {
        let number = *block.header.number();
        let post_hash = block.post_hash();

        let justifications = block.justifications.take();

        debug!(target: "aleph-justification", "Importing block {:?} {:?} {:?}", number, block.header.hash(), block.post_hash());
        let result = self.inner.import_block(block).await;

        if let Ok(ImportResult::Imported(_)) = result {
            if let Some(justification) =
                justifications.and_then(|just| just.into_justification(ALEPH_ENGINE_ID))
            {
                debug!(target: "aleph-justification", "Got justification along imported block {:?}", number);

                if let Err(e) = self.send_justification(
                    BlockId::new(post_hash, number),
                    (ALEPH_ENGINE_ID, justification),
                ) {
                    warn!(target: "aleph-justification", "Error while receiving justification for block {:?}: {:?}", post_hash, e);
                }
            }
        }

        result
    }
}

#[async_trait::async_trait]
impl<I, JT> JustificationImport<Block> for AlephBlockImport<I, JT>
where
    I: BlockImport<Block> + Clone + Send,
    JT: JustificationTranslator,
{
    type Error = ConsensusError;

    async fn on_start(&mut self) -> Vec<(BlockHash, BlockNumber)> {
        debug!(target: "aleph-justification", "On start called");
        Vec::new()
    }

    async fn import_justification(
        &mut self,
        hash: BlockHash,
        number: BlockNumber,
        justification: SubstrateJustification,
    ) -> Result<(), Self::Error> {
        use SendJustificationError::*;
        debug!(target: "aleph-justification", "import_justification called on {:?}", justification);
        self.send_justification(BlockId::new(hash, number), justification)
            .map_err(|error| match error {
                Send(_) => ConsensusError::ClientImport(String::from(
                    "Could not send justification to ConsensusParty",
                )),
                Consensus(e) => *e,
                Decode(e) => ConsensusError::ClientImport(format!(
                    "Justification for block {:?} decoded incorrectly: {}",
                    number, e
                )),
                Translate(e) => ConsensusError::ClientImport(format!(
                    "Could not translate justification: {}",
                    e
                )),
            })
    }
}
