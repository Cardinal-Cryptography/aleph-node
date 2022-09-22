use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{channel::mpsc, Stream, StreamExt};
use futures_timer::Delay;
use log::{debug, error};
use sc_client_api::HeaderBackend;
use sp_api::BlockT;
use sp_runtime::traits::Header;
use tokio::time::timeout;

use crate::{
    finalization::BlockFinalizer,
    justification::{
        requester::BlockRequester, JustificationHandlerConfig, JustificationNotification,
        JustificationRequestScheduler, SessionInfo, SessionInfoProvider, Verifier,
    },
    network, Metrics, STATUS_REPORT_INTERVAL,
};

pub struct JustificationHandler<B, V, RB, C, S, SI, F>
where
    B: BlockT,
    V: Verifier<B>,
    RB: network::RequestBlocks<B> + 'static,
    C: HeaderBackend<B> + Send + Sync + 'static,
    S: JustificationRequestScheduler,
    SI: SessionInfoProvider<B, V>,
    F: BlockFinalizer<B>,
{
    session_info_provider: SI,
    block_requester: BlockRequester<B, RB, C, S, F, V>,
    verifier_timeout: Duration,
    notification_timeout: Duration,
}

impl<B, V, RB, C, S, SI, F> JustificationHandler<B, V, RB, C, S, SI, F>
where
    B: BlockT,
    V: Verifier<B>,
    RB: network::RequestBlocks<B> + 'static,
    C: HeaderBackend<B> + Send + Sync + 'static,
    S: JustificationRequestScheduler,
    SI: SessionInfoProvider<B, V>,
    F: BlockFinalizer<B>,
{
    pub fn new(
        session_info_provider: SI,
        block_requester: RB,
        client: Arc<C>,
        finalizer: F,
        justification_request_scheduler: S,
        metrics: Option<Metrics<<B::Header as Header>::Hash>>,
        justification_handler_config: JustificationHandlerConfig<B>,
    ) -> Self {
        Self {
            session_info_provider,
            block_requester: BlockRequester::new(
                block_requester,
                client,
                finalizer,
                justification_request_scheduler,
                metrics,
                justification_handler_config.min_allowed_delay,
            ),
            verifier_timeout: justification_handler_config.verifier_timeout,
            notification_timeout: justification_handler_config.notification_timeout,
        }
    }

    pub async fn run(
        mut self,
        authority_justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
        import_justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    ) {
        let import_stream = wrap_channel_with_logging(import_justification_rx, "import");
        let authority_stream = wrap_channel_with_logging(authority_justification_rx, "aggregator");
        let mut notification_stream = futures::stream::select(import_stream, authority_stream);
        let mut last_status_report = Instant::now();

        loop {
            let last_finalized_number = self.block_requester.finalized_number();
            let SessionInfo {
                verifier,
                last_block_height: stop_h,
                current_session,
            } = self
                .session_info_provider
                .for_block_num(last_finalized_number + 1u32.into())
                .await;
            if verifier.is_none() {
                debug!(target: "aleph-justification", "Verifier for session {:?} not yet available. Waiting {}ms and will try again ...", current_session, self.verifier_timeout.as_millis());
                Delay::new(self.verifier_timeout).await;
                continue;
            }
            let verifier = verifier.expect("We loop until this is some.");

            match timeout(self.notification_timeout, notification_stream.next()).await {
                Ok(Some(notification)) => {
                    self.block_requester.handle_justification_notification(
                        notification,
                        verifier,
                        last_finalized_number,
                        stop_h,
                    );
                }
                Ok(None) => panic!("Justification stream ended."),
                Err(_) => {} //Timeout passed
            }

            self.block_requester.request_justification(stop_h);
            if Instant::now().saturating_duration_since(last_status_report)
                >= STATUS_REPORT_INTERVAL
            {
                self.block_requester.status_report();
                last_status_report = Instant::now();
            }
        }
    }
}

fn wrap_channel_with_logging<B: BlockT>(
    channel: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    label: &'static str,
) -> impl Stream<Item = JustificationNotification<B>> {
    channel
        .inspect(move |_| {
            debug!(target: "aleph-justification", "Got justification ({})", label);
        })
        .chain(futures::stream::iter(std::iter::from_fn(move || {
            error!(target: "aleph-justification", "Justification ({}) stream ended.", label);
            None
        })))
}
