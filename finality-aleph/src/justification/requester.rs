use std::{fmt, marker::PhantomData, time::Instant};

use aleph_primitives::BlockNumber;
use log::{debug, error, info, warn};

use crate::{
    finalization::BlockFinalizer,
    justification::{
        scheduler::SchedulerActions, JustificationNotification, JustificationRequestScheduler,
        Verifier,
    },
    metrics::Checkpoint,
    network, BlockIdentifier, BlockchainBackend, ChainInfo, Metrics,
};

/// Threshold for how many tries are needed so that JustificationRequestStatus is logged
const REPORT_THRESHOLD: u32 = 2;

/// This structure is created for keeping and reporting status of BlockRequester
pub struct JustificationRequestStatus<BI: BlockIdentifier> {
    block_hash_number: Option<BI>,
    block_tries: u32,
    parent: Option<BI::Hash>,
    n_children: usize,
    children_tries: u32,
    report_threshold: u32,
}

impl<BI: BlockIdentifier> JustificationRequestStatus<BI> {
    fn new() -> Self {
        Self {
            block_hash_number: None,
            block_tries: 0,
            parent: None,
            n_children: 0,
            children_tries: 0,
            report_threshold: REPORT_THRESHOLD,
        }
    }

    fn save_children(&mut self, hash: BI::Hash, n_children: usize) {
        if self.parent == Some(hash) {
            self.children_tries += 1;
        } else {
            self.parent = Some(hash);
            self.children_tries = 1;
        }
        self.n_children = n_children;
    }

    fn save_block(&mut self, hn: BI) {
        if self.block_hash_number == Some(hn.clone()) {
            self.block_tries += 1;
        } else {
            self.block_hash_number = Some(hn);
            self.block_tries = 1;
        }
    }

    fn reset(&mut self) {
        self.block_hash_number = None;
        self.block_tries = 0;
        self.parent = None;
        self.n_children = 0;
        self.children_tries = 0;
    }

    fn should_report(&self) -> bool {
        self.block_tries >= self.report_threshold || self.children_tries >= self.report_threshold
    }
}

impl<BI: BlockIdentifier> fmt::Display for JustificationRequestStatus<BI> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.block_tries >= self.report_threshold {
            if let Some(hn) = &self.block_hash_number {
                write!(
                    f,
                    "tries - {}; requested block number - {}; hash - {};",
                    self.block_tries,
                    hn.number(),
                    hn.block_hash(),
                )?;
            }
        }
        if self.children_tries >= self.report_threshold {
            if let Some(parent) = self.parent {
                write!(
                    f,
                    "tries - {}; requested {} children of finalized block {}; ",
                    self.children_tries, self.n_children, parent
                )?;
            }
        }
        Ok(())
    }
}

pub struct BlockRequester<BI, RB, S, F, V, BB>
where
    BI: BlockIdentifier,
    RB: network::RequestBlocks<BI> + 'static,
    S: JustificationRequestScheduler,
    F: BlockFinalizer<BI>,
    V: Verifier<BI>,
    BB: BlockchainBackend<BI> + 'static,
{
    block_requester: RB,
    blockchain_backend: BB,
    finalizer: F,
    justification_request_scheduler: S,
    metrics: Option<Metrics<BI::Hash>>,
    request_status: JustificationRequestStatus<BI>,
    _phantom: PhantomData<V>,
}

impl<BI, RB, S, F, V, BB> BlockRequester<BI, RB, S, F, V, BB>
where
    BI: BlockIdentifier,
    RB: network::RequestBlocks<BI> + 'static,
    S: JustificationRequestScheduler,
    F: BlockFinalizer<BI>,
    V: Verifier<BI>,
    BB: BlockchainBackend<BI> + 'static,
{
    pub fn new(
        block_requester: RB,
        blockchain_backend: BB,
        finalizer: F,
        justification_request_scheduler: S,
        metrics: Option<Metrics<BI::Hash>>,
    ) -> Self {
        BlockRequester {
            block_requester,
            blockchain_backend,
            finalizer,
            justification_request_scheduler,
            metrics,
            request_status: JustificationRequestStatus::new(),
            _phantom: PhantomData,
        }
    }

    pub fn handle_justification_notification(
        &mut self,
        notification: JustificationNotification<BI>,
        verifier: V,
        last_finalized: BlockNumber,
        stop_h: BlockNumber,
    ) {
        let JustificationNotification {
            justification,
            block_id,
        } = notification;

        let number = block_id.number();
        let hash = block_id.block_hash();

        if number <= last_finalized || number > stop_h {
            debug!(target: "aleph-justification", "Not finalizing block {:?}. Last finalized {:?}, stop_h {:?}", number, last_finalized, stop_h);
            return;
        };

        if !(verifier.verify(&justification, hash)) {
            warn!(target: "aleph-justification", "Error when verifying justification for block {:?} {:?}", number, hash);
            return;
        };

        debug!(target: "aleph-justification", "Finalizing block {:?} {:?}", number, hash);
        let finalization_res = self
            .finalizer
            .finalize_block(block_id, justification.into());
        match finalization_res {
            Ok(()) => {
                self.justification_request_scheduler.on_block_finalized();
                self.request_status.reset();
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

    pub fn request_justification(&mut self, wanted: BlockNumber) {
        match self.justification_request_scheduler.schedule_action() {
            SchedulerActions::Request => {
                let info = self.blockchain_backend.info();
                self.request_children(&info);
                self.request_wanted(wanted, &info);
            }
            SchedulerActions::ClearQueue => {
                debug!(target: "aleph-justification", "Clearing justification request queue");
                self.block_requester.clear_justification_requests();
            }
            SchedulerActions::Wait => (),
        }
    }

    pub fn finalized_number(&self) -> BlockNumber {
        self.blockchain_backend.info().finalized_block.number()
    }

    fn do_request(&mut self, block_id: BI) {
        debug!(target: "aleph-justification",
               "We have block {:?} with hash {:?}. Requesting justification.", block_id.number(), block_id.block_hash());
        self.justification_request_scheduler.on_request_sent();
        self.block_requester.request_justification(block_id);
    }

    // We request justifications for all the children of last finalized block.
    // Assuming that we request at the same pace that finalization is progressing, it ensures
    // that we are up to date with finalization.
    // We also request the child that it's on the same branch as top_wanted since a fork may happen
    // somewhere in between them.
    fn request_children(&mut self, info: &ChainInfo<BI>) {
        let finalized_hash = info.finalized_block.block_hash();

        let children = self
            .blockchain_backend
            .children(info.finalized_block.clone());

        if !children.is_empty() {
            self.request_status
                .save_children(finalized_hash, children.len());
        }

        for child in children {
            self.do_request(child);
        }
    }

    // This request is important in the case when we are far behind and want to catch up.
    fn request_wanted(&mut self, mut top_wanted: BlockNumber, info: &ChainInfo<BI>) {
        let best_number = info.best_block.number();
        if best_number <= top_wanted {
            // most probably block best_number is not yet finalized
            top_wanted = best_number.saturating_sub(1);
        }
        let finalized_number = info.finalized_block.number();
        // We know that top_wanted >= finalized_number, so
        // - if top_wanted == finalized_number, then we don't want to request it
        // - if top_wanted == finalized_number + 1, then we already requested it
        if top_wanted <= finalized_number + 1 {
            return;
        }
        match self.blockchain_backend.id(top_wanted) {
            Ok(Some(block_id)) => {
                self.do_request(block_id.clone());
                self.request_status.save_block(block_id);
            }
            Ok(None) => {
                warn!(target: "aleph-justification", "Cancelling request, because we don't have block {:?}.", top_wanted);
            }
            Err(err) => {
                warn!(target: "aleph-justification", "Cancelling request, because fetching block {:?} failed {:?}.", top_wanted, err);
            }
        }
    }
}
