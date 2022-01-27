use crate::{
    finalization::BlockFinalizer,
    justification::{JustificationNotification, JustificationRequestDelay, Verifier},
    metrics::Checkpoint,
    network, Metrics,
};
use aleph_primitives::ALEPH_ENGINE_ID;
use codec::Encode;
use log::{debug, error, warn};
use sc_client_api::HeaderBackend;
use sp_api::{BlockId, BlockT, NumberFor};
use sp_runtime::traits::Header;
use std::{marker::PhantomData, sync::Arc, time::Instant};

pub(crate) struct BlockRequester<B, RB, C, D, F, V>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    C: HeaderBackend<B> + Send + Sync + 'static,
    D: JustificationRequestDelay,
    F: BlockFinalizer<B>,
    V: Verifier<B>,
{
    attempt: u32,
    last_seen_finalized: NumberFor<B>,
    block_requester: RB,
    client: Arc<C>,
    finalizer: F,
    justification_request_delay: D,
    metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    phantom: PhantomData<V>,
}

///Max amount of tries we can not update a finalized block number before we will clear requests queue
const MAX_ATTEMPS: u32 = 5;
///Distance (in amount of blocks) between the best and the block we want to request justification
const MIN_ALLOWED_DELAY: u32 = 3;

impl<B, RB, C, D, F, V> BlockRequester<B, RB, C, D, F, V>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    C: HeaderBackend<B> + Send + Sync + 'static,
    D: JustificationRequestDelay,
    F: BlockFinalizer<B>,
    V: Verifier<B>,
{
    pub fn new(
        block_requester: RB,
        client: Arc<C>,
        finalizer: F,
        justification_request_delay: D,
        metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    ) -> Self {
        BlockRequester {
            attempt: 0,
            last_seen_finalized: 0u32.into(),
            block_requester,
            client,
            finalizer,
            justification_request_delay,
            metrics,
            phantom: PhantomData,
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
            debug!(target: "afa", "Not finalizing block {:?}. Last finalized {:?}, stop_h {:?}", number, last_finalized, stop_h);
            return;
        };

        if !(verifier.verify(&justification, hash)) {
            warn!(target: "afa", "Error when verifying justification for block {:?} {:?}", number, hash);
            return;
        };

        debug!(target: "afa", "Finalizing block {:?} {:?}", number, hash);
        let finalization_res = self.finalizer.finalize_block(
            hash,
            number,
            Some((ALEPH_ENGINE_ID, justification.encode())),
        );
        match finalization_res {
            Ok(()) => {
                self.justification_request_delay.on_block_finalized();
                debug!(target: "afa", "Successfully finalized {:?}", number);
                if let Some(metrics) = &self.metrics {
                    metrics.report_block(hash, Instant::now(), Checkpoint::Finalized);
                }
            }
            Err(e) => {
                error!(target: "afa", "Fail in finalization of {:?} {:?} -- {:?}", number, hash, e);
            }
        }
    }

    fn maintain_requests(&mut self) {
        if self.client.info().finalized_number == self.last_seen_finalized {
            self.attempt += 1;
        } else {
            self.attempt = 0;
            self.last_seen_finalized = self.client.info().finalized_number;
        }

        if self.attempt == MAX_ATTEMPS {
            self.block_requester.clear_justification_requests();
            self.attempt = 0;
        }
    }

    pub fn request_justification(&mut self, num: NumberFor<B>) -> NumberFor<B> {
        self.maintain_requests();

        let num = if num > self.client.info().best_number
            && self.client.info().best_number > MIN_ALLOWED_DELAY.into()
        {
            self.client.info().best_number - MIN_ALLOWED_DELAY.into()
        } else {
            num
        };

        if self.justification_request_delay.can_request_now() {
            debug!(target: "afa", "Trying to request block {:?}", num);

            if let Ok(Some(header)) = self.client.header(BlockId::Number(num)) {
                debug!(target: "afa", "We have block {:?} with hash {:?}. Requesting justification.", num, header.hash());
                self.justification_request_delay.on_request_sent();
                self.block_requester
                    .request_justification(&header.hash(), *header.number());
            } else {
                debug!(target: "afa", "Cancelling request, because we don't have block {:?}.", num);
            }
        }

        self.client.info().finalized_number
    }
}
