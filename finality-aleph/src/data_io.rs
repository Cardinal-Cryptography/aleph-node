use crate::Error;
use futures::{channel::mpsc, prelude::*, StreamExt};
use rush::OrderedBatch;
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};
use std::{
    collections::HashSet,
    pin::Pin,
    task::{Context, Poll},
};

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

pub(crate) struct ProposalSelect<T> {
    receivers: Vec<(u32, mpsc::UnboundedReceiver<T>)>,
    new_receivers_rx: mpsc::UnboundedReceiver<(u32, mpsc::UnboundedReceiver<T>)>,
}

impl<T> ProposalSelect<T> {
    pub(crate) fn new(
        new_receivers_rx: mpsc::UnboundedReceiver<(u32, mpsc::UnboundedReceiver<T>)>,
    ) -> Self {
        Self {
            new_receivers_rx,
            receivers: Vec::new(),
        }
    }
}

impl<T> Stream for ProposalSelect<T> {
    type Item = (u32, T);

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
