use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    marker::Send,
};

use parity_scale_codec::Codec;

mod compatibility;
mod data;
mod forest;
mod handler;
#[cfg(test)]
mod mock;
mod service;
pub mod substrate;
mod task_queue;
mod tasks;
mod ticker;

pub use compatibility::OldSyncCompatibleRequestBlocks;
pub use service::{DatabaseIO, Service};
pub use substrate::{
    Justification as SubstrateJustification, JustificationTranslator, SessionVerifier,
    SubstrateChainStatus, SubstrateChainStatusNotifier, SubstrateFinalizationInfo,
    SubstrateSyncBlock, VerifierCache,
};

use crate::BlockIdentifier;

const LOG_TARGET: &str = "aleph-block-sync";

/// The identifier of a connected peer.
pub trait PeerId: Debug + Clone + Hash + Eq {}

impl<T: Debug + Clone + Hash + Eq> PeerId for T {}

/// The header of a block, containing information about the parent relation.
pub trait Header: Clone + Codec + Debug + Send + Sync + 'static {
    type Identifier: BlockIdentifier;

    /// The identifier of this block.
    fn id(&self) -> Self::Identifier;

    /// The identifier of this block's parent.
    fn parent_id(&self) -> Option<Self::Identifier>;
}

/// The block, including a header.
pub trait Block: Clone + Codec + Debug + Send + Sync + 'static {
    type Header: Header;

    /// The header of the block.
    fn header(&self) -> &Self::Header;
}

/// The block importer.
pub trait BlockImport<B>: Send + 'static {
    /// Import the block.
    fn import_block(&mut self, block: B);
}

type BlockIdFor<J> = <<J as Justification>::Header as Header>::Identifier;

/// The verified justification of a block, including a header.
pub trait Justification: Clone + Send + Sync + Debug + 'static {
    type Header: Header;
    /// The implementation has to behave as if the header here is identical to the one returned by
    /// the `header` method after successful verification.
    type Unverified: Header<Identifier = BlockIdFor<Self>> + Debug;

    /// The header of the block.
    fn header(&self) -> &Self::Header;

    /// Return an unverified version of this, for sending over the network.
    fn into_unverified(self) -> Self::Unverified;
}

/// A verifier of justifications.
pub trait Verifier<J: Justification> {
    type Error: Display;

    /// Verifies the raw justification and returns a full justification if successful, otherwise an
    /// error.
    fn verify(&mut self, justification: J::Unverified) -> Result<J, Self::Error>;
}

/// A facility for finalizing blocks using justifications.
pub trait Finalizer<J: Justification> {
    type Error: Display;

    /// Finalize a block using this justification. Since the justification contains the header, we
    /// don't need to additionally specify the block.
    fn finalize(&self, justification: J) -> Result<(), Self::Error>;
}

/// A notification about the chain status changing.
#[derive(Clone, Debug)]
pub enum ChainStatusNotification<H: Header> {
    /// A block has been imported.
    BlockImported(H),
    /// A block has been finalized.
    BlockFinalized(H),
}

/// A stream of notifications about the chain status in the database changing.
/// We assume that this will return all the events, otherwise we will end up with a broken state.
#[async_trait::async_trait]
pub trait ChainStatusNotifier<H: Header> {
    type Error: Display;

    /// Returns a chain status notification when it is available.
    /// This method's implementation must be cancellation safe.
    async fn next(&mut self) -> Result<ChainStatusNotification<H>, Self::Error>;
}

/// The status of a block in the database.
pub enum BlockStatus<J: Justification> {
    /// The block is justified and thus finalized.
    Justified(J),
    /// The block is present, might be finalized if a descendant is justified.
    Present(J::Header),
    /// The block is not known.
    Unknown,
}

/// FinalizationStatus of the block
pub enum FinalizationStatus<J: Justification> {
    /// The block is finalized by justification.
    FinalizedWithJustification(J),
    /// The block is finalized because one of its children is finalized.
    FinalizedByDescendant(J::Header),
    /// The block is not finalized
    NotFinalized,
}

impl<J: Justification> FinalizationStatus<J> {
    pub fn has_justification(&self) -> Option<J> {
        use FinalizationStatus::*;
        match self {
            FinalizedWithJustification(just) => Some(just.clone()),
            _ => None,
        }
    }
}

/// The knowledge about the chain status.
pub trait ChainStatus<B, J>: Clone + Send + Sync + 'static
where
    J: Justification,
    B: Block<Header = J::Header>,
{
    type Error: Display;

    /// The status of the block.
    fn status_of(&self, id: BlockIdFor<J>) -> Result<BlockStatus<J>, Self::Error>;

    /// Export a copy of the block.
    fn block(&self, id: BlockIdFor<J>) -> Result<Option<B>, Self::Error>;

    /// Export a copy of the header.
    fn header(&self, id: BlockIdFor<J>) -> Result<Option<J::Header>, Self::Error>;

    /// The justification at this block number, if we have it otherwise just block id if
    /// the block is finalized without justification. Should return NotFinalized variant if
    /// the request is above the top finalized.
    fn finalized_at(&self, number: u32) -> Result<FinalizationStatus<J>, Self::Error>;

    /// The header of the best block.
    fn best_block(&self) -> Result<J::Header, Self::Error>;

    /// The justification of the top finalized block.
    fn top_finalized(&self) -> Result<J, Self::Error>;

    /// Children of the specified block.
    fn children(&self, id: BlockIdFor<J>) -> Result<Vec<J::Header>, Self::Error>;
}

/// An interface for submitting additional justifications to the justification sync.
/// Chiefly ones created by ABFT, but others will also be handled appropriately.
/// The block corresponding to the submitted `Justification` MUST be obtained and
/// imported into the Substrate database by the user, as soon as possible.
pub trait JustificationSubmissions<J: Justification>: Clone + Send + 'static {
    type Error: Display;

    /// Submit a justification to the underlying justification sync.
    fn submit(&mut self, justification: J::Unverified) -> Result<(), Self::Error>;
}

/// An interface for requesting specific blocks from the block sync.
/// Required by the data availability mechanism in ABFT.
pub trait RequestBlocks<BI: BlockIdentifier>: Clone + Send + Sync + 'static {
    type Error: Display;

    /// Request the given block.
    fn request_block(&self, block_id: BI) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug)]
pub enum ChainStatusExtError<T, J>
where
    T: Display,
    J: Justification,
{
    Inner(T),
    MissingBlockInPath(BlockIdFor<J>),
    MissingBlockParent(BlockIdFor<J>),
    NoStraightPathBetween(BlockIdFor<J>, BlockIdFor<J>),
}

impl<T, J> From<T> for ChainStatusExtError<T, J>
where
    T: Display,
    J: Justification,
{
    fn from(value: T) -> Self {
        ChainStatusExtError::Inner(value)
    }
}

impl<T, J> Display for ChainStatusExtError<T, J>
where
    T: Display,
    J: Justification,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainStatusExtError::Inner(e) => write!(f, "{}", *e),
            ChainStatusExtError::MissingBlockInPath(id) => write!(f, "Missing block {:?}", id),
            ChainStatusExtError::MissingBlockParent(id) => {
                write!(f, "Missing parent of the block {:?}", id)
            }
            ChainStatusExtError::NoStraightPathBetween(from, to) => {
                write!(f, "{:?} is not an ancestor of {:?}", from, to)
            }
        }
    }
}

/// Possible answers to the question is block `a` ancestor of the `b`.
#[derive(PartialEq)]
pub enum IsAncestor {
    /// Yes, it is.
    Yes,
    /// No, it isn't.
    No,
    /// We don't know.
    Unknown,
}

pub trait ChainStatusExt<B, J>
where
    J: Justification,
    B: Block<Header = J::Header>,
{
    type Error: Display;

    /// Path of blocks between two blocks. Does not include `to` block.
    ///
    /// `to` is an ancestor of the `from`.
    fn block_path(
        &self,
        from: &BlockIdFor<J>,
        to: &BlockIdFor<J>,
    ) -> Result<Vec<B>, ChainStatusExtError<Self::Error, J>>;

    /// Path of headers between two blocks. Does not include `to` header..
    ///
    /// `to` is an ancestor of the `from`.
    fn headers_path(
        &self,
        from: &BlockIdFor<J>,
        to: &BlockIdFor<J>,
    ) -> Result<Vec<B::Header>, ChainStatusExtError<Self::Error, J>>;

    fn is_ancestor_of(&self, ancestor: &BlockIdFor<J>, of: &BlockIdFor<J>) -> IsAncestor;
}

impl<T, B, J> ChainStatusExt<B, J> for T
where
    T: ChainStatus<B, J>,
    J: Justification,
    B: Block<Header = J::Header>,
{
    type Error = T::Error;
    fn block_path(
        &self,
        from: &BlockIdFor<J>,
        to: &BlockIdFor<J>,
    ) -> Result<Vec<B>, ChainStatusExtError<Self::Error, J>> {
        let mut blocks = vec![];
        let headers = self.headers_path(from, to)?;
        for header in headers {
            let block = match self.block(header.id())? {
                Some(b) => b,
                None => return Err(ChainStatusExtError::MissingBlockInPath(header.id())),
            };

            blocks.push(block);
        }

        Ok(blocks)
    }

    fn headers_path(
        &self,
        from: &BlockIdFor<J>,
        to: &BlockIdFor<J>,
    ) -> Result<Vec<B::Header>, ChainStatusExtError<Self::Error, J>> {
        let mut current = from.clone();
        let mut path = vec![];

        while current != *to && current.number() > to.number() {
            let header = match self.header(current.clone())? {
                Some(header) => header,
                None => return Err(ChainStatusExtError::MissingBlockInPath(current)),
            };

            current = match header.parent_id() {
                Some(id) => id,
                None => return Err(ChainStatusExtError::MissingBlockParent(current)),
            };

            path.push(header);
        }

        if current != *to {
            return Err(ChainStatusExtError::NoStraightPathBetween(
                from.clone(),
                to.clone(),
            ));
        }

        Ok(path)
    }

    fn is_ancestor_of(&self, ancestor: &BlockIdFor<J>, of: &BlockIdFor<J>) -> IsAncestor {
        match self.headers_path(of, ancestor) {
            Ok(_) => IsAncestor::Yes,
            Err(ChainStatusExtError::NoStraightPathBetween(..)) => IsAncestor::No,
            _ => IsAncestor::Unknown,
        }
    }
}
