use std::time::{Duration, Instant};

use futures::{channel::mpsc, Stream, StreamExt};
use futures_timer::Delay;
use log::{debug, error};
use sp_api::{BlockT, NumberFor};
use sp_runtime::traits::{Header, UniqueSaturatedInto};
use tokio::time::timeout;

use crate::{
    finalization::BlockFinalizer,
    justification::{
        requester::BlockRequester, JustificationHandlerConfig, JustificationNotification,
        JustificationRequestScheduler, SessionInfo, SessionInfoProvider, Verifier,
    },
    network,
    session::{last_block_of_session, session_id_from_block_num},
    session_map::AuthorityProvider,
    sync::SessionVerifier,
    BlockchainBackend, Metrics, SessionPeriod, STATUS_REPORT_INTERVAL,
};

pub struct JustificationHandler<B, RB, S, F, BB, AP>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    S: JustificationRequestScheduler,
    AP: AuthorityProvider<NumberFor<B>>,
    F: BlockFinalizer<B>,
    BB: BlockchainBackend<B> + 'static,
{
    authority_provider: AP,
    block_requester: BlockRequester<B, RB, S, F, SessionVerifier, BB>,
    verifier_timeout: Duration,
    notification_timeout: Duration,
}

impl<B, RB, S, F, BB, AP> JustificationHandler<B, RB, S, F, BB, AP>
where
    B: BlockT,
    RB: network::RequestBlocks<B> + 'static,
    S: JustificationRequestScheduler,
    AP: AuthorityProvider<NumberFor<B>>,
    F: BlockFinalizer<B>,
    BB: BlockchainBackend<B> + 'static,
{
    pub fn new(
        authority_provider: AP,
        block_requester: RB,
        blockchain_backend: BB,
        finalizer: F,
        justification_request_scheduler: S,
        metrics: Option<Metrics<<B::Header as Header>::Hash>>,
        justification_handler_config: JustificationHandlerConfig,
    ) -> Self {
        Self {
            authority_provider,
            block_requester: BlockRequester::new(
                block_requester,
                blockchain_backend,
                finalizer,
                justification_request_scheduler,
                metrics,
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
            let current_session =
                session_id_from_block_num(last_finalized_number + 1u32.into(), SessionPeriod(900));
            let last_block_height = last_block_of_session(current_session, SessionPeriod(900));
            let verifier = self
                .authority_provider
                .authority_data(last_finalized_number + 1u32.into());
            if verifier.is_none() {
                debug!(target: "aleph-justification", "Verifier for session {:?} not yet available. Waiting {}ms and will try again ...", current_session, self.verifier_timeout.as_millis());
                Delay::new(self.verifier_timeout).await;
                continue;
            }
            let verifier: SessionVerifier = verifier.expect("We loop until this is some.").into();

            match timeout(self.notification_timeout, notification_stream.next()).await {
                Ok(Some(notification)) => {
                    self.block_requester.handle_justification_notification(
                        notification,
                        verifier,
                        last_finalized_number,
                        last_block_height,
                    );
                }
                Ok(None) => panic!("Justification stream ended."),
                Err(_) => {} //Timeout passed
            }

            let mut wanted = Vec::new();
            for x in (UniqueSaturatedInto::<u32>::unique_saturated_into(last_block_height)
                ..UniqueSaturatedInto::<u32>::unique_saturated_into(
                    self.block_requester.best_number(),
                ))
                .step_by(900)
                .take(20)
            {
                wanted.push(x.into());
            }

            self.block_requester.request_justification(wanted);
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
