use std::{fmt, marker::PhantomData, sync::Arc, time::Instant};

use aleph_primitives::ALEPH_ENGINE_ID;
use log::{debug, error, info, warn};
use sc_client_api::{backend::Backend, blockchain::Backend as _, HeaderBackend};
use sp_api::{BlockId, BlockT, NumberFor};
use sp_runtime::traits::{Header, One};

use crate::{
    finalization::BlockFinalizer,
    justification::{
        scheduler::SchedulerActions, versioned_encode, JustificationNotification,
        JustificationRequestScheduler, Verifier,
    },
    metrics::Checkpoint,
    network, Metrics,
};

/// Threshold for how many tries are needed so that JustificationRequestStatus is logged
const REPORT_THRESHOLD: u32 = 2;

/// This structure is created for keeping and reporting status of BlockRequester
pub struct JustificationRequestStatus<B: BlockT> {
    block_number: Option<NumberFor<B>>,
    block_hash: Option<B::Hash>,
    block_tries: u32,
    parent: Option<B::Hash>,
    n_childred: Option<usize>,
    children_tries: u32,
    report_threshold: u32,
}

impl<B: BlockT> JustificationRequestStatus<B> {
    fn new() -> Self {
        Self {
            block_number: None,
            block_hash: None,
            block_tries: 0,
            parent: None,
            n_childred: None,
            children_tries: 0,
            report_threshold: REPORT_THRESHOLD,
        }
    }

    fn save_children(&mut self, hash: B::Hash, n_childred: usize) {
        if self.parent == Some(hash) {
            self.children_tries += 1;
        } else {
            self.parent = Some(hash);
            self.n_childred = Some(n_childred);
            self.children_tries = 1;
        }
    }

    fn save_block(&mut self, num: NumberFor<B>, hash: B::Hash) {
        if self.block_number == Some(num) {
            self.block_tries += 1;
        } else {
            self.block_hash = Some(hash);
            self.block_number = Some(num);
            self.block_hash = None;
            self.block_tries = 1;
        }
    }

    fn should_report(&self) -> bool {
        self.block_tries >= self.report_threshold || self.children_tries >= self.report_threshold
    }
}

impl<B: BlockT> fmt::Display for JustificationRequestStatus<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.block_tries >= self.report_threshold {
            if let Some(n) = self.block_number {
                let mut status = format!(
                    "tries - {}; requested block number - {}; ",
                    self.block_tries, n
                );
                if let Some(header) = self.block_hash {
                    status.push_str(&format!("hash - {}; ", header));
                } else {
                    status.push_str("hash - unknown; ");
                }

                write!(f, "{}", status)?;
            }
        }
        if self.children_tries >= self.report_threshold {
            if let Some(parent) = self.parent {
                let n = self
                    .n_childred
                    .expect("Children number is saved with parent.");

                write!(
                    f,
                    "tries - {}; requested {} children of finalized block {}; ",
                    self.children_tries, n, parent
                )?;
            }
        }
        Ok(())
    }
}

pub struct BlockRequester<B, RB, S, F, V, BE>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    S: JustificationRequestScheduler,
    F: BlockFinalizer<B>,
    V: Verifier<B>,
    BE: Backend<B>,
{
    block_requester: RB,
    backend: Arc<BE>,
    finalizer: F,
    justification_request_scheduler: S,
    metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    request_status: JustificationRequestStatus<B>,
    _phantom: PhantomData<V>,
}

impl<B, RB, S, F, V, BE> BlockRequester<B, RB, S, F, V, BE>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    S: JustificationRequestScheduler,
    F: BlockFinalizer<B>,
    V: Verifier<B>,
    BE: Backend<B>,
{
    pub fn new(
        block_requester: RB,
        backend: Arc<BE>,
        finalizer: F,
        justification_request_scheduler: S,
        metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    ) -> Self {
        BlockRequester {
            block_requester,
            backend,
            finalizer,
            justification_request_scheduler,
            metrics,
            request_status: JustificationRequestStatus::new(),
            _phantom: PhantomData,
        }
    }

    pub fn handle_justification_notification(
        &mut self,
        notification: JustificationNotification<B>,
        verifier: V,
        last_finalized: NumberFor<B>,
        stop_h: NumberFor<B>,
    ) {
        let JustificationNotification {
            justification,
            number,
            hash,
        } = notification;

        if number <= last_finalized || number > stop_h {
            debug!(target: "aleph-justification", "Not finalizing block {:?}. Last finalized {:?}, stop_h {:?}", number, last_finalized, stop_h);
            return;
        };

        if !(verifier.verify(&justification, hash)) {
            warn!(target: "aleph-justification", "Error when verifying justification for block {:?} {:?}", number, hash);
            return;
        };

        debug!(target: "aleph-justification", "Finalizing block {:?} {:?}", number, hash);
        let finalization_res = self.finalizer.finalize_block(
            hash,
            number,
            Some((ALEPH_ENGINE_ID, versioned_encode(justification))),
        );
        match finalization_res {
            Ok(()) => {
                self.justification_request_scheduler.on_block_finalized();
                debug!(target: "aleph-justification", "Successfully finalized {:?}", number);
                if let Some(metrics) = &self.metrics {
                    metrics.report_block(hash, Instant::now(), Checkpoint::Finalized);
                }
            }
            Err(e) => {
                error!(target: "aleph-justification", "Fail in finalization of {:?} {:?} -- {:?}", number, hash, e);
            }
        }
    }

    pub fn status_report(&self) {
        if self.request_status.should_report() {
            info!(target: "aleph-justification", "Justification requester status report: {}", self.request_status);
        }
    }

    pub fn request_justification(&mut self, num: NumberFor<B>) {
        match self.justification_request_scheduler.schedule_action() {
            SchedulerActions::Request => {
                self.request_targets(num)
                    .into_iter()
                    .for_each(|header| self.request(header));
            }
            SchedulerActions::ClearQueue => {
                debug!(target: "aleph-justification", "Clearing justification request queue");
                self.block_requester.clear_justification_requests();
            }
            SchedulerActions::Wait => (),
        }
    }

    pub fn finalized_number(&self) -> NumberFor<B> {
        self.backend.blockchain().info().finalized_number
    }

    fn request(&mut self, header: <B as BlockT>::Header) {
        let number = *header.number();
        let hash = header.hash();
        debug!(target: "aleph-justification",
               "We have block {:?} with hash {:?}. Requesting justification.", number, hash);
        self.justification_request_scheduler.on_request_sent();
        self.block_requester.request_justification(&hash, number);
    }

    // We request justifications for all the children of last finalized block and a justification
    // for a block of number num on longest branch.
    // Assuming that we request at the same pace that finalization is progressing, the former ensures
    // that we are up to date with finalization. On the other hand, the former enables fast catch up
    // if we are behind.
    // We don't remove the child that it's on the same branch as best since a fork may happen
    // somewhere in between them.
    fn request_targets(&mut self, mut top_wanted: NumberFor<B>) -> Vec<<B as BlockT>::Header> {
        let blockchain_backend = self.backend.blockchain();
        let blockchain_info = blockchain_backend.info();
        let finalized_hash = blockchain_info.finalized_hash;

        let mut targets = blockchain_backend
            .children(finalized_hash)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|hash| {
                if let Ok(Some(header)) = blockchain_backend.header(BlockId::Hash(hash)) {
                    Some(header)
                } else {
                    warn!(target: "aleph-justification",
                      "Cancelling request for child {:?} of {:?}: no block.", hash, finalized_hash);
                    None
                }
            })
            .collect::<Vec<_>>();
        self.request_status
            .save_children(finalized_hash, targets.len());
        let best_number = blockchain_info.best_number;
        if best_number <= top_wanted {
            // most probably block best_number is not yet finalized
            top_wanted = best_number - NumberFor::<B>::one();
        }
        match blockchain_backend.header(BlockId::Number(top_wanted)) {
            Ok(Some(header)) => {
                if !targets.contains(&header) {
                    self.request_status
                        .save_block(*header.number(), header.hash());
                    targets.push(header);
                }
            }
            Ok(None) => {
                warn!(target: "aleph-justification", "Cancelling request, because we don't have block {:?}.", top_wanted);
            }
            Err(err) => {
                warn!(target: "aleph-justification", "Cancelling request, because fetching block {:?} failed {:?}.", top_wanted, err);
            }
        }

        targets
    }
}
