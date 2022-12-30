use std::fmt::{Display, Error as FmtError, Formatter};

use aleph_primitives::BlockNumber;
use futures::StreamExt;
use sc_client_api::client::{FinalityNotifications, ImportNotifications};
use sp_runtime::traits::{Block as BlockT, Header as SubstrateHeader};
use tokio::select;

use crate::sync::{substrate::BlockId, ChainStatusNotification, ChainStatusNotifier, Header};

/// What can go wrong when waiting for next chain status notification.
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

/// Substrate specific implementation of `ChainStatusNotifier`.
pub struct SubstrateChainStatusNotifier<H, B>
where
    H: SubstrateHeader<Number = BlockNumber>,
    B: BlockT<Header = H>,
{
    finality_notifications: FinalityNotifications<B>,
    import_notifications: ImportNotifications<B>,
}

impl<H, B> SubstrateChainStatusNotifier<H, B>
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
impl<H, B> ChainStatusNotifier<BlockId<H>> for SubstrateChainStatusNotifier<H, B>
where
    H: SubstrateHeader<Number = BlockNumber>,
    B: BlockT<Header = H>,
{
    type Error = Error;

    /// Returns next chain status notification.
    async fn next(&mut self) -> Result<ChainStatusNotification<BlockId<H>>, Self::Error> {
        select! {
            maybe_block = self.finality_notifications.next() => {
                maybe_block
                    .map(|block| ChainStatusNotification::BlockFinalized(block.header.id()))
                    .ok_or(Error::JustificationStream)
            },
            maybe_block = self.import_notifications.next() => {
                maybe_block
                .map(|block| ChainStatusNotification::BlockImported(block.header.id()))
                .ok_or(Error::ImportStream)
            }
        }
    }
}
