use crate::{
    justification::{AlephJustification, JustificationNotification},
    metrics::Metrics,
};
use codec::Decode;
use futures::channel::mpsc::{TrySendError, UnboundedSender};
use log::debug;
use sc_client_api::backend::Backend;
use sp_api::TransactionFor;
use sp_consensus::{
    BlockCheckParams, BlockImport, BlockImportParams, Error as ConsensusError, ImportResult,
    JustificationImport,
};
use sp_runtime::{
    traits::{Block as BlockT, Header, NumberFor},
    Justification,
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc, time::Instant};

pub struct AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be>,
{
    inner: Arc<I>,
    justification_tx: UnboundedSender<JustificationNotification<Block>>,
    metrics: Option<Metrics<Block::Header>>,
    _phantom: PhantomData<Be>,
}

enum SendJustificationError<Block>
where
    Block: BlockT,
{
    Send(TrySendError<JustificationNotification<Block>>),
    Decode,
}

impl<Block, Be, I> AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be>,
{
    pub fn new(
        inner: Arc<I>,
        justification_tx: UnboundedSender<JustificationNotification<Block>>,
        metrics: Option<Metrics<Block::Header>>,
    ) -> AlephBlockImport<Block, Be, I> {
        AlephBlockImport {
            inner,
            justification_tx,
            metrics,
            _phantom: PhantomData,
        }
    }

    fn send_justification(
        &mut self,
        hash: Block::Hash,
        number: NumberFor<Block>,
        justification: Justification,
    ) -> Result<(), SendJustificationError<Block>> {
        debug!(target: "afa", "Importing justification for block #{:?}", number);

        let aleph_justification = AlephJustification::decode(&mut &*justification)
            .map_err(|_| SendJustificationError::Decode)?;

        self.justification_tx
            .unbounded_send(JustificationNotification {
                hash,
                number,
                justification: aleph_justification,
            })
            .map_err(SendJustificationError::Send)
    }
}

impl<Block, Be, I> Clone for AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be>,
{
    fn clone(&self) -> Self {
        AlephBlockImport {
            inner: self.inner.clone(),
            justification_tx: self.justification_tx.clone(),
            metrics: self.metrics.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<Block, Be, I> BlockImport<Block> for AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be> + Send,
    for<'a> &'a I:
        BlockImport<Block, Error = ConsensusError, Transaction = TransactionFor<I, Block>>,
    TransactionFor<I, Block>: Send + 'static,
{
    type Error = <I as BlockImport<Block>>::Error;
    type Transaction = TransactionFor<I, Block>;

    fn check_block(&mut self, block: BlockCheckParams<Block>) -> Result<ImportResult, Self::Error> {
        self.inner.check_block(block)
    }

    fn import_block(
        &mut self,
        mut block: BlockImportParams<Block, Self::Transaction>,
        cache: HashMap<[u8; 4], Vec<u8>>,
    ) -> Result<ImportResult, Self::Error> {
        let number = *block.header.number();
        let post_hash = block.post_hash();
        if let Some(m) = &self.metrics {
            m.report_block(post_hash, Instant::now(), "importing");
        };

        let justification = block.justification.take();

        debug!(target: "afa", "Importing block {:?} {:?} {:?}", number, block.header.hash(), block.post_hash());
        let import_result = self.inner.import_block(block, cache);

        let imported_aux = match import_result {
            Ok(ImportResult::Imported(aux)) => aux,
            Ok(r) => return Ok(r),
            Err(e) => return Err(e),
        };

        debug!(target: "afa", "Got justification along imported block #{:?}", number);

        if let Some(justification) = justification {
            if self
                .send_justification(post_hash, number, justification)
                .is_err()
            {
                debug!(target: "afa", "Some issue with justification");
            }
        }

        if let Some(m) = &self.metrics {
            m.report_block(post_hash, Instant::now(), "imported");
        };

        Ok(ImportResult::Imported(imported_aux))
    }
}

impl<Block, Be, I> JustificationImport<Block> for AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be>,
{
    type Error = ConsensusError;

    fn on_start(&mut self) -> Vec<(Block::Hash, NumberFor<Block>)> {
        debug!(target: "afa", "On start called");
        Vec::new()
    }

    fn import_justification(
        &mut self,
        hash: Block::Hash,
        number: NumberFor<Block>,
        justification: Justification,
    ) -> Result<(), Self::Error> {
        debug!(target: "afa", "import_justification called on {:?}", justification);
        self.send_justification(hash, number, justification)
            .map_err(|error| match error {
                SendJustificationError::Send(_) => ConsensusError::ClientImport(String::from(
                    "Could not send justification to ConsensusParty",
                )),
                SendJustificationError::Decode => {
                    ConsensusError::ClientImport(String::from("Could not decode justification"))
                }
            })
    }
}
