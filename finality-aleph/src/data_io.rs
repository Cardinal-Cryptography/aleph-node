use crate::{Error, Metrics};
use aleph_bft::OrderedBatch;
use futures::{
    channel::{mpsc, oneshot},
    stream::StreamExt,
};
use log::debug;
use parking_lot::Mutex;
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct DataIO<B: Block> {
    pub(crate) best_chain: Arc<Mutex<B::Hash>>,
    pub(crate) ordered_batch_tx: mpsc::UnboundedSender<OrderedBatch<B::Hash>>,
    pub(crate) refresh_best_chain_tx: mpsc::UnboundedSender<()>,
    pub(crate) metrics: Option<Metrics<B::Header>>,
}

pub(crate) async fn refresh_best_chain<B: Block, SC: SelectChain<B>>(
    mut requests: mpsc::UnboundedReceiver<()>,
    select_chain: SC,
    best_chain: Arc<Mutex<B::Hash>>,
    mut exit: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = requests.next() => {
                let new_best_chain = select_chain
                    .best_chain()
                    .await
                    .expect("No best chain")
                    .hash();
                *best_chain.lock() = new_best_chain;
            }
            _ = &mut exit => {
                debug!(target: "afa", "Task for refreshing best chain received exit signal. Terminating.");
                return;
            }
        }
    }
}

impl<B: Block> aleph_bft::DataIO<B::Hash> for DataIO<B> {
    type Error = Error;

    fn get_data(&self) -> B::Hash {
        if self.refresh_best_chain_tx.unbounded_send(()).is_err() {
            debug!(target: "afa", "Channel for new best chain requests is closed.");
        }

        let hash = *self.best_chain.lock();

        if let Some(m) = &self.metrics {
            m.report_block(hash, std::time::Instant::now(), "get_data");
        }
        debug!(target: "afa", "Outputting {:?} in get_data", hash);
        hash
    }

    fn send_ordered_batch(&mut self, batch: OrderedBatch<B::Hash>) -> Result<(), Self::Error> {
        // TODO: add better conversion
        self.ordered_batch_tx
            .unbounded_send(batch)
            .map_err(|_| Error::SendData)
    }
}
