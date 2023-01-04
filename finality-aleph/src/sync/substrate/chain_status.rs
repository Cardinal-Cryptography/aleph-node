use std::{
    fmt::{Display, Error as FmtError, Formatter},
    marker::PhantomData,
};

use aleph_primitives::{BlockNumber, ALEPH_ENGINE_ID};
use sp_blockchain::Backend;
use sp_runtime::{
    generic::BlockId as SubstrateBlockId,
    traits::{Block as BlockT, Header as SubstrateHeader},
};

use crate::{
    justification::{backwards_compatible_decode, DecodeError},
    sync::{substrate::Justification, BlockStatus, ChainStatus, Header},
};

/// What can go wrong when checking chain status
#[derive(Debug)]
pub enum Error<B: BlockT> {
    
    JustificationDecode(DecodeError),
    MissingHash(B::Hash),
    MissingJustification(B::Hash),
}

impl<B: BlockT> Display for Error<B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            JustificationDecode(error) => {
                write!(f, "could not decode the stored justification: {}", error)
            }
            MissingHash(hash) => {
                write!(f, "no block for hash {:?}", hash)
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

impl<B: BlockT> From<DecodeError> for Error<B> {
    fn from(value: DecodeError) -> Self {
        Error::JustificationDecode(value)
    }
}

/// Substrate implementation of ChainStatus trait
struct SubstrateChainStatus<B, BE>
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

    fn justification(&self, hash: B::Hash) -> Option<Vec<u8>> {
        let id = SubstrateBlockId::<B>::Hash(hash);
        self.client
            .justifications(id)
            .expect("substrate client must respond")
            .map(|j| j.into_justification(ALEPH_ENGINE_ID))
            .flatten()
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
    ) -> Result<BlockStatus<Justification<B::Header>>, Self::Error> {
        let header = match self.header(id.hash) {
            Some(header) => header,
            None => return Ok(BlockStatus::Unknown),
        };

        if let Some(justification) = self.justification(id.hash) {
            Ok(BlockStatus::Justified(Justification {
                header,
                raw_justification: backwards_compatible_decode(justification)?,
            }))
        } else {
            Ok(BlockStatus::Present(header))
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
        let justification = self
            .justification(finalized_hash)
            .ok_or(Error::MissingJustification(finalized_hash))?;

        Ok(Justification {
            header,
            raw_justification: backwards_compatible_decode(justification)?,
        })
    }
}
