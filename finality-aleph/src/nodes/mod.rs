mod consensus_node;
mod nonvalidator_node;

pub use consensus_node::run_consensus_node;
pub use nonvalidator_node::run_nonvalidator_node;

use crate::crypto::AuthorityVerifier;
use crate::finalization::AlephFinalizer;
use crate::justification::{
    JustificationHandler, JustificationRequestSchedulerImpl, SessionInfo, SessionInfoProvider,
};
use crate::mpsc::UnboundedSender;
use crate::session_map::ReadOnlySessionMap;
use crate::{
    last_block_of_session, mpsc, session_id_from_block_num, JustificationNotification, Metrics,
    MillisecsPerBlock, SessionPeriod,
};
use sc_client_api::Backend;
use sc_network::{ExHashT, NetworkService};
use sp_runtime::traits::{Block, Header, NumberFor};
use std::future::Future;
use std::sync::Arc;

/// Max amount of tries we can not update a finalized block number before we will clear requests queue
const MAX_ATTEMPTS: u32 = 5;

pub struct AlephParams<B: Block, H: ExHashT, C, SC> {
    pub config: crate::AlephConfig<B, H, C, SC>,
}

pub struct JustificationParams<B: Block, H: ExHashT, C> {
    pub network: Arc<NetworkService<B, H>>,
    pub client: Arc<C>,
    pub justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    pub metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    pub session_period: SessionPeriod,
    pub millisecs_per_block: MillisecsPerBlock,
    pub session_map: ReadOnlySessionMap,
}

struct SessionInfoProviderImpl {
    session_authorities: ReadOnlySessionMap,
    session_period: SessionPeriod,
}

impl SessionInfoProviderImpl {
    fn new(session_authorities: ReadOnlySessionMap, session_period: SessionPeriod) -> Self {
        Self {
            session_authorities,
            session_period,
        }
    }
}

#[async_trait::async_trait]
impl<B: Block> SessionInfoProvider<B, AuthorityVerifier> for SessionInfoProviderImpl {
    async fn for_block_num(&self, number: NumberFor<B>) -> SessionInfo<B, AuthorityVerifier> {
        let current_session = session_id_from_block_num::<B>(number, self.session_period);
        let last_block_height = last_block_of_session::<B>(current_session, self.session_period);
        let verifier = self
            .session_authorities
            .get(current_session)
            .await
            .map(AuthorityVerifier::new);

        SessionInfo {
            current_session,
            last_block_height,
            verifier,
        }
    }
}

pub fn setup_justification_handler<B, H, C, BE>(
    just_params: JustificationParams<B, H, C>,
) -> (
    UnboundedSender<JustificationNotification<B>>,
    impl Future<Output = ()>,
)
where
    B: Block,
    H: ExHashT,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    BE: Backend<B> + 'static,
{
    let JustificationParams {
        network,
        client,
        justification_rx,
        metrics,
        session_period,
        millisecs_per_block,
        session_map,
    } = just_params;

    let handler = JustificationHandler::new(
        SessionInfoProviderImpl::new(session_map, session_period),
        network,
        client.clone(),
        AlephFinalizer::new(client),
        JustificationRequestSchedulerImpl::new(&session_period, &millisecs_per_block, MAX_ATTEMPTS),
        metrics,
        Default::default(),
    );

    let (authority_justification_tx, authority_justification_rx) = mpsc::unbounded();
    (authority_justification_tx, async move {
        handler
            .run(authority_justification_rx, justification_rx)
            .await;
    })
}
