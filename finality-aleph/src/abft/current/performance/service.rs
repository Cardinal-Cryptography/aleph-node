use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use log::{debug, warn};

use crate::{
    abft::{current::performance::Batch, LOG_TARGET},
    data_io::AlephData,
    party::manager::Runnable,
    Hasher, UnverifiedHeader,
};

struct FinalizationWrapper<UH, FH>
where
    UH: UnverifiedHeader,
    FH: current_aleph_bft::FinalizationHandler<AlephData<UH>>,
{
    finalization_handler: FH,
    batches_for_scorer: mpsc::UnboundedSender<Batch<UH>>,
}

impl<UH, FH> FinalizationWrapper<UH, FH>
where
    UH: UnverifiedHeader,
    FH: current_aleph_bft::FinalizationHandler<AlephData<UH>>,
{
    fn new(finalization_handler: FH, batches_for_scorer: mpsc::UnboundedSender<Batch<UH>>) -> Self {
        FinalizationWrapper {
            finalization_handler,
            batches_for_scorer,
        }
    }
}

impl<UH, FH> current_aleph_bft::UnitFinalizationHandler for FinalizationWrapper<UH, FH>
where
    UH: UnverifiedHeader,
    FH: current_aleph_bft::FinalizationHandler<AlephData<UH>>,
{
    type Data = AlephData<UH>;
    type Hasher = Hasher;

    fn batch_finalized(&mut self, batch: Batch<UH>) {
        for unit in &batch {
            if let Some(data) = &unit.data {
                self.finalization_handler.data_finalized(data.clone())
            }
        }
        if let Err(err) = self.batches_for_scorer.unbounded_send(batch) {
            warn!(target: LOG_TARGET, "Failed to send ABFT batch to performance scoring: {}.", err);
        }
    }
}

/// A service computing the performance score of ABFT nodes based on batches of ordered units.
pub struct Service<UH>
where
    UH: UnverifiedHeader,
{
    batches_from_abft: mpsc::UnboundedReceiver<Batch<UH>>,
}

impl<UH> Service<UH>
where
    UH: UnverifiedHeader,
{
    /// Create a new service, together with a unit finalizaiton handler that should be passed to
    /// ABFT. It will wrap the provided finalization handler and call it in the background.
    pub fn new<FH>(
        finalization_handler: FH,
    ) -> (
        Self,
        impl current_aleph_bft::UnitFinalizationHandler<Data = AlephData<UH>, Hasher = Hasher>,
    )
    where
        FH: current_aleph_bft::FinalizationHandler<AlephData<UH>>,
    {
        let (batches_for_us, batches_from_abft) = mpsc::unbounded();
        (
            Service { batches_from_abft },
            FinalizationWrapper::new(finalization_handler, batches_for_us),
        )
    }
}

#[async_trait::async_trait]
impl<UH> Runnable for Service<UH>
where
    UH: UnverifiedHeader,
{
    async fn run(mut self, mut exit: oneshot::Receiver<()>) {
        loop {
            tokio::select! {
                _maybe_batch = self.batches_from_abft.next() => {
                    // TODO(A0-4575): actually compute the score form batches etc
                }
                _ = &mut exit => {
                    debug!(target: LOG_TARGET, "ABFT performance scoring task received exit signal. Terminating.");
                    break;
                }
            }
        }
    }
}
