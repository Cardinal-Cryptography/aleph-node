use std::fmt::{Display, Error as FmtError, Formatter};

use aleph_primitives::BlockNumber;
use futures::StreamExt;
use sc_client_api::client::{FinalityNotifications, ImportNotifications};
use sp_runtime::traits::{Block as BlockT, Header as SubstrateHeader};
use tokio::select;

use crate::sync::{substrate::BlockId, ChainStateNotification, ChainStateNotifier, Header};

/// What can go wrong when waiting for next chain state notification.
#[derive(Debug)]
pub enum Error {
    JustificationStream,
    ImportStream,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            JustificationStream => {
                write!(f, "finalization notification stream has ended")
            }
            ImportStream => {
                write!(f, "import notification stream has ended")
            }
        }
    }
}

/// Substrate specific implementation of `ChainStateNotifier`.
pub struct SubstrateChainStateNotifier<H, B>
where
    H: SubstrateHeader<Number = BlockNumber>,
    B: BlockT<Header = H>,
{
    finality_notifications: FinalityNotifications<B>,
    import_notifications: ImportNotifications<B>,
}

impl<H, B> SubstrateChainStateNotifier<H, B>
where
    H: SubstrateHeader<Number = BlockNumber>,
    B: BlockT<Header = H>,
{
    fn new(
        finality_notifications: FinalityNotifications<B>,
        import_notifications: ImportNotifications<B>,
    ) -> Self {
        Self {
            finality_notifications,
            import_notifications,
        }
    }
}

#[async_trait::async_trait]
impl<H, B> ChainStateNotifier<BlockId<H>> for SubstrateChainStateNotifier<H, B>
where
    H: SubstrateHeader<Number = BlockNumber>,
    B: BlockT<Header = H>,
{
    type Error = Error;

    /// Returns next chain state notification.
    async fn next(&mut self) -> Result<ChainStateNotification<BlockId<H>>, Self::Error> {
        select! {
            maybe_block = self.finality_notifications.next() => {
                maybe_block
                    .map(|block| ChainStateNotification::BlockFinalized(block.header.id()))
                    .ok_or(Error::JustificationStream)
            },
            maybe_block = self.import_notifications.next() => {
                maybe_block
                .map(|block| ChainStateNotification::BlockImported(block.header.id()))
                .ok_or(Error::ImportStream)
            }
        }
    }
}
