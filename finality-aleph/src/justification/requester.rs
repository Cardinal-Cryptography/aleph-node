use std::{fmt, marker::PhantomData, time::Instant};

use aleph_primitives::ALEPH_ENGINE_ID;
use log::{debug, error, info, warn};
use sc_client_api::{backend::Backend, HeaderBackend};
use sp_api::{BlockId, BlockT, NumberFor};
use sp_runtime::traits::Header;

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
    tries: u32,
    report_threshold: u32,
}

impl<B: BlockT> JustificationRequestStatus<B> {
    fn new() -> Self {
        Self {
            block_number: None,
            block_hash: None,
            tries: 0,
            report_threshold: REPORT_THRESHOLD,
        }
    }

    fn save_block_number(&mut self, num: NumberFor<B>) {
        if Some(num) == self.block_number {
            self.tries += 1;
        } else {
            self.block_number = Some(num);
            self.block_hash = None;
            self.tries = 1;
        }
    }

    fn insert_hash(&mut self, hash: B::Hash) {
        self.block_hash = Some(hash);
    }

    fn should_report(&self) -> bool {
        self.tries >= self.report_threshold
    }
}

impl<B: BlockT> fmt::Display for JustificationRequestStatus<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(n) = self.block_number {
            let mut status = format!("tries - {}; requested block number - {}; ", self.tries, n);
            if let Some(header) = self.block_hash {
                status.push_str(&format!("hash - {}; ", header));
            } else {
                status.push_str("hash - unknown; ");
            }

            write!(f, "{}", status)?;
        }
        Ok(())
    }
}

pub struct BlockRequester<B, RB, S, F, V, BB>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    S: JustificationRequestScheduler,
    F: BlockFinalizer<B>,
    V: Verifier<B>,
{
    block_requester: RB,
    backend: BB,
    finalizer: F,
    justification_request_scheduler: S,
    metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    min_allowed_delay: NumberFor<B>,
    request_status: JustificationRequestStatus<B>,
    _phantom: PhantomData<V>,
}

impl<B, RB, S, F, V, BB> BlockRequester<B, RB, S, F, V, BB>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    S: JustificationRequestScheduler,
    F: BlockFinalizer<B>,
    V: Verifier<B>,
    BB: Backend<B>,
{
    pub fn new(
        block_requester: RB,
        backend: BB,
        finalizer: F,
        justification_request_scheduler: S,
        metrics: Option<Metrics<<B::Header as Header>::Hash>>,
        min_allowed_delay: NumberFor<B>,
    ) -> Self {
        BlockRequester {
            block_requester,
            backend,
            finalizer,
            justification_request_scheduler,
            metrics,
            min_allowed_delay,
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
                let finalized_hash = self.backend.blockchain().info().finalized_hash;

                let best_number = self.backend.blockchain().info().best_number;
                let finalized_number = self.backend.blockchain().info().finalized_number;
                let delay = if self.min_allowed_delay < (best_number - finalized_number) {
                    self.min_allowed_delay
                } else {
                    best_number - finalized_number
                };
                let num = if num > best_number && best_number > self.min_allowed_delay {
                    best_number - delay
                } else {
                    num
                };

                debug!(target: "aleph-justification", "Trying to request block {:?}", num);
                self.request_status.save_block_number(num);

                if let Ok(Some(header)) = self.backend.blockchain().header(BlockId::Number(num)) {
                    self.request_status.insert_hash(header.hash());
                    debug!(target: "aleph-justification", "We have block {:?} with hash {:?}. Requesting justification.", num, header.hash());
                    self.justification_request_scheduler.on_request_sent();
                    self.block_requester
                        .request_justification(&header.hash(), *header.number());
                } else {
                    debug!(target: "aleph-justification", "Cancelling request, because we don't have block {:?}.", num);
                }
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
}
