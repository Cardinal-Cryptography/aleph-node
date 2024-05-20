use std::sync::Arc;

use log::{error, info, warn};
use network_clique::SpawnHandleT;
use parking_lot::RwLock;
use sp_consensus::{Error, SelectChain};
use sp_runtime::traits::Block as BlockT;
use tokio::sync::mpsc::{channel, error::TrySendError, Receiver, Sender};

use crate::{Block, BlockHash, SpawnHandle};

// It never should grow that much
const CHANNEL_SIZE: u32 = 1024;
const LOG_TARGET: &str = "aleph-select-chain";

pub fn select_chain_state_handler<B: Block>(
    select_chain: FavouriteSelectChain<B>,
    spawn_handle: &SpawnHandle,
) -> SelectChainStateHandler<B::Header> {
    let (rx, tx) = channel(CHANNEL_SIZE as usize);

    let favourite = select_chain.favourite;

    let mut state_writer = StateWriter::<B> {
        events: tx,
        favourite,
    };

    spawn_handle.spawn("aleph/state_writer", async move {
        info!(target: LOG_TARGET, "starting up StateWriter");
        state_writer.run().await;
        error!(target: LOG_TARGET, "StateWriter unexpectly finished");
    });

    SelectChainStateHandler { favourite_sender: rx }
}

pub struct SelectChainStateHandler<Header> {
    favourite_sender: Sender<Header>,
}

impl<Header> SelectChainStateHandler<Header> {
    pub fn update_favourite(&self, new_favourite: Header) {
        match self
            .favourite_sender
            .try_send(new_favourite)
        {
            Err(TrySendError::Full(_)) => {
                warn!(target: LOG_TARGET, "SelectChainStateHandler channel full, skipping one notification")
            }
            _ => (),
        }
    }
}

pub struct StateWriter<B: Block> {
    events: Receiver<<B as Block>::Header>,
    favourite: Arc<RwLock<<B as Block>::Header>>,
}

impl<B: Block> StateWriter<B> {
    async fn run(&mut self) {
        loop {
            let new = self
                .events
                .recv()
                .await
                .expect("we hold the second end & its never ending stream.");
            self.update_favourite(new);
        }
    }

    fn update_favourite(&self, new_favourite: <B as Block>::Header) {
        *self.favourite.write() = new_favourite;
    }
}

#[derive(Clone)]
pub struct FavouriteSelectChain<B: Block> {
    favourite: Arc<RwLock<<B as Block>::Header>>,
}

impl<B: Block> FavouriteSelectChain<B> {
    pub fn new(favourite: <B as Block>::Header) -> Self {
        Self {
            favourite: Arc::new(RwLock::new(favourite)),
        }
    }
}

#[async_trait::async_trait]
impl<B, H> SelectChain<B> for FavouriteSelectChain<B>
where
    B: BlockT<Header = H, Hash = BlockHash>,
    B: Block<Header = H, Hash = BlockHash>,
    H: Sync + Send + Clone,
{
    async fn leaves(&self) -> Result<Vec<<B as BlockT>::Hash>, Error> {
        // this is never used in the current version
        Ok(vec![])
    }

    async fn best_chain(&self) -> Result<<B as BlockT>::Header, Error> {
        let best = self.favourite.read();

        Ok(best.clone())
    }
}
