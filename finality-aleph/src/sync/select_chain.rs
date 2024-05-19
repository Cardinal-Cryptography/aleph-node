use std::{collections::HashSet, sync::Arc};

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
) -> SelectChainStateHandler<B::Header, B::Hash> {
    let (rx, tx) = channel(CHANNEL_SIZE as usize);

    let leaves = select_chain.leaves;
    let favourite = select_chain.favourite;

    let mut state_writer = StateWriter::<B> {
        events: tx,
        leaves,
        favourite,
    };

    spawn_handle.spawn(
        "aleph/state_writer",
        async move {
            info!(target: LOG_TARGET, "starting up StateWriter");
            state_writer.run().await;
            error!(target: LOG_TARGET, "StateWriter unexpectly finished");
        },
    );

    SelectChainStateHandler { events_sender: rx }
}

enum Events<Header, Hash> {
    NewFavourite(Header),
    NewLeave(Hash, Hash),
    PruneLeave(Hash),
}

pub struct SelectChainStateHandler<Header, Hash> {
    events_sender: Sender<Events<Header, Hash>>,
}

impl<Header, Hash> SelectChainStateHandler<Header, Hash> {
    pub fn update_favourite(&self, new_favourite: Header) {
        match self
            .events_sender
            .try_send(Events::NewFavourite(new_favourite))
        {
            Err(TrySendError::Full(_)) => {
                warn!(target: LOG_TARGET, "SelectChainStateHandler channel full, skipping one notification")
            }
            _ => (),
        }
    }

    pub fn update_leaves(&self, new: Hash, to_remove: Hash) {
        match self
            .events_sender
            .try_send(Events::NewLeave(new, to_remove))
        {
            Err(TrySendError::Full(_)) => {
                warn!(target: LOG_TARGET, "SelectChainStateHandler channel full, skipping one notification")
            }
            _ => (),
        }
    }

    pub fn remove(&self, to_prune: Hash) {
        match self.events_sender.try_send(Events::PruneLeave(to_prune)) {
            Err(TrySendError::Full(_)) => {
                warn!(target: LOG_TARGET, "SelectChainStateHandler channel full, skipping one notification")
            }
            _ => (),
        }
    }
}

pub struct StateWriter<B: Block> {
    events: Receiver<Events<<B as Block>::Header, <B as Block>::Hash>>,
    favourite: Arc<RwLock<<B as Block>::Header>>,
    leaves: Arc<RwLock<HashSet<<B as Block>::Hash>>>,
}

impl<B: Block> StateWriter<B> {
    async fn run(&mut self) {
        loop {
            match self
                .events
                .recv()
                .await
                .expect("we hold the second end & its never ending stream.")
            {
                Events::NewFavourite(new) => self.update_favourite(new),
                Events::NewLeave(new, to_remove) => self.update_leaves(new, to_remove),
                Events::PruneLeave(leave) => self.prune_leaves(leave),
            }
        }
    }

    fn update_favourite(&self, new_favourite: <B as Block>::Header) {
        *self.favourite.write() = new_favourite;
    }

    fn update_leaves(&self, new: <B as Block>::Hash, to_remove: <B as Block>::Hash) {
        let mut leaves = self.leaves.write();
        leaves.insert(new);
        leaves.remove(&to_remove);
    }

    fn prune_leaves(&self, to_prune: <B as Block>::Hash) {
        let mut leaves = self.leaves.write();
        leaves.remove(&to_prune);
    }
}

#[derive(Clone)]
pub struct FavouriteSelectChain<B: Block> {
    favourite: Arc<RwLock<<B as Block>::Header>>,
    leaves: Arc<RwLock<HashSet<<B as Block>::Hash>>>,
}

impl<B: Block> FavouriteSelectChain<B> {
    pub fn new(favourite: <B as Block>::Header) -> Self {
        Self {
            leaves: Arc::new(RwLock::new(HashSet::new())),
            favourite: Arc::new(RwLock::new(favourite)),
        }
    }

    fn leaves(&self) -> Vec<<B as Block>::Hash> {
        let set = self.leaves.read();

        Vec::from_iter(set.clone())
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
        let leaves = self.leaves();

        Ok(leaves)
    }

    async fn best_chain(&self) -> Result<<B as BlockT>::Header, Error> {
        let best = self.favourite.read();

        Ok(best.clone())
    }
}
