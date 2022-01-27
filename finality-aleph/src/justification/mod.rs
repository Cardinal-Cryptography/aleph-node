use crate::{crypto::Signature, finalization::BlockFinalizer, network, Metrics, SessionId};
use aleph_bft::SignatureSet;
use codec::{Decode, Encode};
use futures::{channel::mpsc, Stream, StreamExt};
use futures_timer::Delay;
use log::{debug, error};
use sc_client_api::HeaderBackend;
use sp_api::{BlockT, NumberFor};
use sp_runtime::traits::Header;
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;

mod backward;
mod requester;

pub(crate) use backward::{backwards_compatible_decode, JustificationDecoding};
use requester::BlockRequester;

/// A proof of block finality, currently in the form of a sufficiently long list of signatures.
#[derive(Clone, Encode, Decode, Debug, PartialEq)]
pub struct AlephJustification {
    pub(crate) signature: SignatureSet<Signature>,
}

pub(crate) trait Verifier<B: BlockT> {
    fn verify(&self, justification: &AlephJustification, hash: B::Hash) -> bool;
}

/// Bunch of methods for managing frequency of sending justification requests.
pub(crate) trait JustificationRequestDelay {
    /// Decides whether enough time has elapsed.
    fn can_request_now(&self) -> bool;
    /// Notice block finalization.
    fn on_block_finalized(&mut self);
    /// Notice request sending.
    fn on_request_sent(&mut self);
}

pub(crate) struct SessionInfo<B: BlockT, V: Verifier<B>> {
    pub(crate) current_session: SessionId,
    pub(crate) last_block_height: NumberFor<B>,
    pub(crate) verifier: Option<V>,
}

/// Returns `SessionInfo` for the session regarding block with no. `number`.
pub(crate) trait SessionInfoProvider<B: BlockT, V: Verifier<B>> {
    fn for_block_num(&self, number: NumberFor<B>) -> SessionInfo<B, V>;
}

impl<F, B, V> SessionInfoProvider<B, V> for F
where
    B: BlockT,
    V: Verifier<B>,
    F: Fn(NumberFor<B>) -> SessionInfo<B, V>,
{
    fn for_block_num(&self, number: NumberFor<B>) -> SessionInfo<B, V> {
        self(number)
    }
}

/// A notification for sending justifications over the network.
#[derive(Clone)]
pub struct JustificationNotification<Block: BlockT> {
    /// The justification itself.
    pub justification: AlephJustification,
    /// The hash of the finalized block.
    pub hash: Block::Hash,
    /// The ID of the finalized block.
    pub number: NumberFor<Block>,
}

pub(crate) struct JustificationHandler<B, V, RB, C, D, SI, F>
where
    B: BlockT,
    V: Verifier<B>,
    RB: network::RequestBlocks<B> + 'static,
    C: HeaderBackend<B> + Send + Sync + 'static,
    D: JustificationRequestDelay,
    SI: SessionInfoProvider<B, V>,
    F: BlockFinalizer<B>,
{
    session_info_provider: SI,
    block_requester: BlockRequester<B, RB, C, D, F, V>,
    /// How long should we wait when the session verifier is not yet available.
    pub(crate) verifier_timeout: Duration,
    /// How long should we wait for any notification.
    pub(crate) notification_timeout: Duration,
}

impl<B, V, RB, C, D, SI, F> JustificationHandler<B, V, RB, C, D, SI, F>
where
    B: BlockT,
    V: Verifier<B>,
    RB: network::RequestBlocks<B> + 'static,
    C: HeaderBackend<B> + Send + Sync + 'static,
    D: JustificationRequestDelay,
    SI: SessionInfoProvider<B, V>,
    F: BlockFinalizer<B>,
{
    pub(crate) fn new(
        session_info_provider: SI,
        block_requester: RB,
        client: Arc<C>,
        finalizer: F,
        justification_request_delay: D,
        metrics: Option<Metrics<<B::Header as Header>::Hash>>,
        verifier_timeout: Duration,
        notification_timeout: Duration,
    ) -> Self {
        Self {
            session_info_provider,
            block_requester: BlockRequester::new(
                block_requester,
                client,
                finalizer,
                justification_request_delay,
                metrics,
            ),
            verifier_timeout,
            notification_timeout,
        }
    }

    pub(crate) async fn run(
        mut self,
        authority_justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
        import_justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    ) {
        let import_stream = wrap_channel_with_logging(import_justification_rx, "import");
        let authority_stream = wrap_channel_with_logging(authority_justification_rx, "aggregator");
        let mut notification_stream = futures::stream::select(import_stream, authority_stream);

        let mut last_finalized_number = 0u32.into();
        loop {
            let SessionInfo {
                verifier,
                last_block_height: stop_h,
                current_session,
            } = self
                .session_info_provider
                .for_block_num(last_finalized_number + 1u32.into());
            if verifier.is_none() {
                debug!(target: "afa", "Verifier for session {:?} not yet available. Waiting {}ms and will try again ...", current_session, self.verifier_timeout.as_millis());
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

            last_finalized_number = self.block_requester.request_justification(stop_h);
        }
    }
}

fn wrap_channel_with_logging<B: BlockT>(
    channel: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    label: &'static str,
) -> impl Stream<Item = JustificationNotification<B>> {
    channel
        .inspect(move |_| {
            debug!(target: "afa", "Got justification ({})", label);
        })
        .chain(futures::stream::iter(std::iter::from_fn(move || {
            error!(target: "afa", "Justification ({}) stream ended.", label);
            None
        })))
}
