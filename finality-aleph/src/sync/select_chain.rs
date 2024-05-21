use std::fmt::Debug;

use futures::channel::{mpsc, oneshot};
use log::debug;
use sp_consensus::{Error, SelectChain};
use sp_runtime::traits::Block as BlockT;

use crate::{Block, BlockHash};

const LOG_TARGET: &str = "aleph-select-chain";

#[derive(Clone)]
pub struct FavouriteSelectChain<B: Block> {
    favourite_block_request: mpsc::UnboundedSender<oneshot::Sender<B::Header>>,
}

impl<B: Block> FavouriteSelectChain<B> {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<oneshot::Sender<B::Header>>) {
        let (rx, tx) = mpsc::unbounded();

        (
            Self {
                favourite_block_request: rx,
            },
            tx,
        )
    }
}

#[async_trait::async_trait]
impl<B, H> SelectChain<B> for FavouriteSelectChain<B>
where
    B: BlockT<Header = H, Hash = BlockHash>,
    B: Block<Header = H, Hash = BlockHash>,
    H: Sync + Send + Clone + Debug,
{
    async fn leaves(&self) -> Result<Vec<<B as BlockT>::Hash>, Error> {
        // this is never used in the current version
        Ok(Vec::new())
    }

    async fn best_chain(&self) -> Result<<B as BlockT>::Header, Error> {
        let (rx, tx) = oneshot::channel();

        self.favourite_block_request
            .unbounded_send(rx)
            .expect("The second end of the channel should be operational");
        let best = tx
            .await
            .expect("The second end of the channel should be operational");

        debug!(target: LOG_TARGET, "Best chain: {:?}", best);

        Ok(best.clone())
    }
}
