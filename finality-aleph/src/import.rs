use crate::{environment::finalize_block, justification::AlephJustification};
use codec::{Decode, Encode};
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
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

pub struct AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be>,
{
    inner: Arc<I>,
    _phantom: PhantomData<(Be, Block)>,
}

impl<Block, Be, I> AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be>,
{
    pub fn new(inner: Arc<I>) -> AlephBlockImport<Block, Be, I> {
        AlephBlockImport {
            inner,
            _phantom: PhantomData,
        }
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
            _phantom: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<Block, Be, I> BlockImport<Block> for AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be> + Send,
    for<'a> &'a I:
        BlockImport<Block, Error = ConsensusError, Transaction = TransactionFor<I, Block>>,
{
    type Error = <I as BlockImport<Block>>::Error;
    type Transaction = TransactionFor<I, Block>;

    async fn check_block(
        &mut self,
        block: BlockCheckParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        self.inner.check_block(block).await
    }

    async fn import_block(
        &mut self,
        mut block: BlockImportParams<Block, Self::Transaction>,
        cache: HashMap<[u8; 4], Vec<u8>>,
    ) -> Result<ImportResult, Self::Error> {
        let number = *block.header.number();
        let hash = block.header.hash();
        let justification = block.justification.take();

        let import_result = self.inner.import_block(block, cache).await;
        let mut imported_aux = match import_result {
            Ok(ImportResult::Imported(aux)) => aux,
            Ok(r) => return Ok(r),
            Err(e) => return Err(e),
        };

        if let Some(justification) = justification {
            let res = self.import_justification(hash, number, justification).await;
            res.unwrap_or_else(|_err| {
                imported_aux.bad_justification = true;
                imported_aux.needs_justification = true;
            });
        }

        Ok(ImportResult::Imported(imported_aux))
    }
}


#[async_trait::async_trait]
impl<Block, Be, I> JustificationImport<Block> for AlephBlockImport<Block, Be, I>
where
    Block: BlockT,
    Be: Backend<Block>,
    I: crate::ClientForAleph<Block, Be> + Send,
{
    type Error = ConsensusError;

    async fn on_start(&mut self) -> Vec<(Block::Hash, NumberFor<Block>)> {
        log::debug!(target: "afg", "On start called");
        Vec::new()
    }

    async fn import_justification(
        &mut self,
        hash: Block::Hash,
        number: NumberFor<Block>,
        justification: Justification,
    ) -> Result<(), Self::Error> {
        log::debug!(target: "afg", "Importing justification for block #{:?}", number);

        if let Ok(justification) = AlephJustification::decode(&mut &*justification) {
            log::debug!(target: "afg", "Finalizing block #{:?} from justification import", number);
            finalize_block(Arc::clone(&self.inner), hash, Some(justification.encode()));
            Ok(())
        } else {
            Err(ConsensusError::ClientImport("Bad justification".into()))
        }
    }
}
