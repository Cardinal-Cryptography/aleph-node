use crate::{party::NumberOps, Error};
use futures::{channel::mpsc, prelude::*, StreamExt};
use log::{debug, error};
use rush::OrderedBatch;
use sc_client_api::backend::Backend;
use sp_api::NumberFor;
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};
use std::{
    collections::HashSet,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub(crate) struct BlockFinalizer<C, B: Block, BE> {
    client: Arc<C>,
    ordered_batch_rx: mpsc::UnboundedReceiver<OrderedBatch<B::Hash>>,
    proposition_tx: mpsc::UnboundedSender<B::Hash>,
    phantom: PhantomData<BE>,
}

impl<C, B: Block, BE> BlockFinalizer<C, B, BE>
where
    BE: Backend<B>,
    C: crate::ClientForAleph<B, BE>,
    NumberFor<B>: NumberOps,
{
    pub(crate) fn new(
        client: Arc<C>,
        ordered_batch_rx: mpsc::UnboundedReceiver<OrderedBatch<B::Hash>>,
        proposition_tx: mpsc::UnboundedSender<B::Hash>,
    ) -> Self {
        BlockFinalizer {
            client,
            ordered_batch_rx,
            proposition_tx,
            phantom: PhantomData,
        }
    }

    fn check_extends_finalized(&self, h: B::Hash) -> bool {
        let head_finalized = self.client.info().finalized_hash;
        if h == head_finalized {
            return false;
        }
        let lca = sp_blockchain::lowest_common_ancestor(self.client.as_ref(), h, head_finalized)
            .expect("No lowest common ancestor");
        lca.hash == head_finalized
    }

    pub(crate) async fn run(mut self) {
        while let Some(batch) = self.ordered_batch_rx.next().await {
            for block_hash in batch {
                if self.check_extends_finalized(block_hash) {
                    debug!(target: "afa", "Sending proposition to finalize block hash {}.", block_hash);
                    self.proposition_tx
                        .unbounded_send(block_hash)
                        .expect("Proposing new blocks should succeed");
                }
            }
        }
        error!(target: "afa", "Voter batch stream closed.");
    }
}

#[derive(Clone)]
pub(crate) struct DataIO<B: Block, SC: SelectChain<B>> {
    pub(crate) select_chain: SC,
    pub(crate) ordered_batch_tx: mpsc::UnboundedSender<OrderedBatch<B::Hash>>,
}

impl<B: Block, SC: SelectChain<B>> rush::DataIO<B::Hash> for DataIO<B, SC> {
    type Error = Error;

    fn get_data(&self) -> B::Hash {
        self.select_chain
            .best_chain()
            .expect("No best chain")
            .hash()
    }

    fn send_ordered_batch(&mut self, batch: OrderedBatch<B::Hash>) -> Result<(), Self::Error> {
        // TODO: add better conversion
        self.ordered_batch_tx
            .unbounded_send(batch)
            .map_err(|_| Error::SendData)
    }
}

pub(crate) struct ProposalSelect<B: Block> {
    receivers: Vec<(u64, mpsc::UnboundedReceiver<B::Hash>)>,
    new_receivers_rx: mpsc::UnboundedReceiver<(u64, mpsc::UnboundedReceiver<B::Hash>)>,
}

impl<B: Block> ProposalSelect<B> {
    pub(crate) fn new(
        new_receivers_rx: mpsc::UnboundedReceiver<(u64, mpsc::UnboundedReceiver<B::Hash>)>,
    ) -> Self {
        Self {
            new_receivers_rx,
            receivers: Vec::new(),
        }
    }
}

impl<B: Block> Stream for ProposalSelect<B> {
    type Item = (u64, B::Hash);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        // Accept new receivers
        match this.new_receivers_rx.poll_next_unpin(cx) {
            Poll::Ready(Some((session_id, session_tx))) => {
                this.receivers.push((session_id, session_tx));
            }
            Poll::Ready(None) => {}
            _ => {}
        }

        let mut ids_to_remove = HashSet::new();
        let mut res = None;

        for (id, receiver) in this.receivers.iter_mut() {
            match receiver.poll_next_unpin(cx) {
                Poll::Ready(Some(ordered_batch)) => {
                    res = Some(Poll::Ready(Some((*id, ordered_batch))));
                    break;
                }
                Poll::Ready(None) => {
                    ids_to_remove.insert(*id);
                }
                Poll::Pending => {}
            }
        }

        this.receivers.retain(|(id, _)| !ids_to_remove.contains(id));

        match res {
            None => Poll::Pending,
            Some(res) => res,
        }
    }
}
