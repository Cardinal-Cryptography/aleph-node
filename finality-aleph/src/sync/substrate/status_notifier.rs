use std::{fmt::{Display, Error as FmtError, Formatter}, time::{Duration, Instant}};

use aleph_primitives::BlockNumber;
use futures::StreamExt;
use sc_client_api::client::{FinalityNotifications, ImportNotifications};
use sp_runtime::traits::{Block as BlockT, Header as SubstrateHeader};
use tokio::{select, time::sleep};

use crate::sync::{ChainStatus, Header, BlockIdentifier, ChainStatusNotification, ChainStatusNotifier, SubstrateChainStatus, substrate::chain_status::Error as ChainStatusError};

/// What can go wrong when waiting for next chain status notification.
#[derive(Debug)]
pub enum Error<B>
where
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    JustificationStreamClosed,
    ImportStreamClosed,
    ChainStatusError(ChainStatusError<B>),
    MajorSyncFallback,
}

impl<B> Display for Error<B>
where
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            JustificationStreamClosed => {
                write!(f, "finalization notification stream has ended")
            }
            ImportStreamClosed => {
                write!(f, "import notification stream has ended")
            }
            ChainStatusError(e) => {
                write!(f, "chain status error: {}", e)
            }
            MajorSyncFallback => {
                write!(f, "waited too long, falling back to manual reporting")
            }
        }
    }
}

/// Substrate specific implementation of `ChainStatusNotifier`.
pub struct SubstrateChainStatusNotifier<B>
where
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    finality_notifications: FinalityNotifications<B>,
    import_notifications: ImportNotifications<B>,
    // The things below here are a hack to ensure all blocks get to the user, even during a major
    // sync. They should almost surely be removed after A0-1760.
    backend: SubstrateChainStatus<B>,
    last_reported: BlockNumber,
    trying_since: Instant,
    catching_up: bool,
}

impl<B> SubstrateChainStatusNotifier<B>
where
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    pub fn new(
        finality_notifications: FinalityNotifications<B>,
        import_notifications: ImportNotifications<B>,
        backend: SubstrateChainStatus<B>,
    ) -> Result<Self, ChainStatusError<B>> {
        let last_reported = backend.best_block()?.id().number();
        Ok(Self {
            finality_notifications,
            import_notifications,
            backend,
            last_reported,
            trying_since: Instant::now(),
            catching_up: false,
        })
    }

    fn header_at(&self, number: BlockNumber) -> Result<Option<B::Header>, ChainStatusError<B>> {
        match self.backend.hash_for_number(number)? {
            Some(hash) => Ok(self.backend.header_for_hash(hash)?),
            None => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl<B> ChainStatusNotifier<B::Header> for SubstrateChainStatusNotifier<B>
where
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    type Error = Error<B>;

    async fn next(&mut self) -> Result<ChainStatusNotification<B::Header>, Self::Error> {
        if self.catching_up {
            match self.header_at(self.last_reported + 1).map_err(Error::ChainStatusError)? {
                Some(header) => {
                    self.last_reported += 1;
                    return Ok(ChainStatusNotification::BlockImported(header));
                },
                None => {
                    self.catching_up = false;
                    self.trying_since = Instant::now();
                },
            }
        }
        select! {
            maybe_block = self.finality_notifications.next() => {
                self.trying_since = Instant::now();
                maybe_block
                    .map(|block| ChainStatusNotification::BlockFinalized(block.header))
                    .ok_or(Error::JustificationStreamClosed)
            },
            maybe_block = self.import_notifications.next() => {
                if let Some(block) = &maybe_block {
                    let number = block.header.id().number();
                    if number > self.last_reported {
                        self.last_reported = number;
                    }
                }
                self.trying_since = Instant::now();
                maybe_block
                .map(|block| ChainStatusNotification::BlockImported(block.header))
                .ok_or(Error::ImportStreamClosed)
            },
            _ = sleep(Duration::from_secs(3).saturating_sub(Instant::now() - self.trying_since)) => {
                self.catching_up = true;
                Err(Error::MajorSyncFallback)
            }
        }
    }
}
