use std::{
    fmt::{Display, Error as FmtError, Formatter},
    marker::PhantomData,
};

use aleph_primitives::{BlockNumber, ALEPH_ENGINE_ID};
use log::warn;
use sp_blockchain::Backend;
use sp_runtime::{
    generic::BlockId as SubstrateBlockId,
    traits::{Block as BlockT, Header as SubstrateHeader},
};

use crate::{
    justification::backwards_compatible_decode,
    sync::{substrate::Justification, BlockStatus, ChainStatus, Header, LOG_TARGET},
    AlephJustification,
};

/// What can go wrong when checking chain status
#[derive(Debug)]
pub enum Error<B: BlockT> {
    MissingHash(B::Hash),
    MissingJustification(B::Hash),
}

impl<B: BlockT> Display for Error<B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            MissingHash(hash) => {
                write!(f, "no block for existing hash {:?}", hash)
            }
            MissingJustification(hash) => {
                write!(
                    f,
                    "no justification for finalized block with hash {:?}",
                    hash
                )
            }
        }
    }
}

/// Substrate implementation of ChainStatus trait
pub struct SubstrateChainStatus<B, BE>
where
    BE: Backend<B>,
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    client: BE,
    _phantom: PhantomData<B>,
}

impl<B, BE> SubstrateChainStatus<B, BE>
where
    BE: Backend<B>,
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    fn header(&self, hash: B::Hash) -> Option<B::Header> {
        let id = SubstrateBlockId::<B>::Hash(hash);
        self.client
            .header(id)
            .expect("substrate client must respond")
    }

    fn justification(&self, hash: B::Hash) -> Option<AlephJustification> {
        let id = SubstrateBlockId::<B>::Hash(hash);
        let justification = self
            .client
            .justifications(id)
            .expect("substrate client must respond")
            .and_then(|j| j.into_justification(ALEPH_ENGINE_ID))?;

        match backwards_compatible_decode(justification) {
            Ok(justification) => Some(justification),
            // This should not happen, as we only import correctly encoded justification.
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Could not decode stored justification for block {:?}: {}", hash, e
                );
                None
            }
        }
    }

    fn best_hash(&self) -> B::Hash {
        self.client.info().best_hash
    }

    fn finalized_hash(&self) -> B::Hash {
        self.client.info().finalized_hash
    }
}

impl<B, BE> ChainStatus<Justification<B::Header>> for SubstrateChainStatus<B, BE>
where
    BE: Backend<B>,
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    type Error = Error<B>;

    fn status_of(
        &self,
        id: <B::Header as Header>::Identifier,
    ) -> BlockStatus<Justification<B::Header>> {
        let header = match self.header(id.hash) {
            Some(header) => header,
            None => return BlockStatus::Unknown,
        };

        if let Some(raw_justification) = self.justification(id.hash) {
            BlockStatus::Justified(Justification {
                header,
                raw_justification,
            })
        } else {
            BlockStatus::Present(header)
        }
    }

    fn best_block(&self) -> Result<B::Header, Self::Error> {
        let best_hash = self.best_hash();

        self.header(best_hash).ok_or(Error::MissingHash(best_hash))
    }

    fn top_finalized(&self) -> Result<Justification<B::Header>, Self::Error> {
        let finalized_hash = self.finalized_hash();

        let header = self
            .header(finalized_hash)
            .ok_or(Error::MissingHash(finalized_hash))?;
        let raw_justification = self
            .justification(finalized_hash)
            .ok_or(Error::MissingJustification(finalized_hash))?;

        Ok(Justification {
            header,
            raw_justification,
        })
    }
}
