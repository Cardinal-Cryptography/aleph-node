use crate::{
    first_block_of_session, session_id_from_block_num, ClientForAleph, SessionId, SessionPeriod,
};
use aleph_primitives::{AlephSessionApi, AuthorityId};
use futures::StreamExt;
use log::debug;
use sc_client_api::{Backend, FinalityNotification};
use sp_runtime::generic::BlockId;
use sp_runtime::traits::{Block, Header, NumberFor};
use sp_runtime::SaturatedConversion;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type SessionMap = HashMap<SessionId, Vec<AuthorityId>>;

#[derive(Clone)]
struct SharedSessionMap(Arc<Mutex<SessionMap>>);

#[derive(Clone)]
pub struct ReadOnlySessionMap(Arc<Mutex<SessionMap>>);

impl SharedSessionMap {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }

    async fn update(
        &mut self,
        id: SessionId,
        authorities: Vec<AuthorityId>,
    ) -> Option<Vec<AuthorityId>> {
        self.0.lock().await.insert(id, authorities)
    }

    async fn prune_below(&mut self, id: SessionId) {
        self.0.lock().await.retain(|&s, _| s >= id);
    }

    fn read_only(&self) -> ReadOnlySessionMap {
        ReadOnlySessionMap(self.0.clone())
    }
}

impl ReadOnlySessionMap {
    pub async fn get(&self, id: SessionId) -> Option<Vec<AuthorityId>> {
        self.0.lock().await.get(&id).cloned()
    }
}

fn get_authorities_for_session<B, C>(
    runtime_api: sp_api::ApiRef<C::Api>,
    session_id: SessionId,
    first_block: NumberFor<B>,
) -> Vec<AuthorityId>
where
    B: Block,
    C: sp_api::ProvideRuntimeApi<B>,
    C::Api: aleph_primitives::AlephSessionApi<B>,
{
    if session_id == SessionId(0) {
        runtime_api
            .authorities(&BlockId::Number(<NumberFor<B>>::saturated_from(0u32)))
            .expect("Authorities for the session 0 must be available from the beginning")
    } else {
        runtime_api
            .next_session_authorities(&BlockId::Number(first_block))
            .unwrap_or_else(|_| {
                panic!(
                    "We didn't get the authorities for the session {:?}",
                    session_id
                )
            })
            .expect(
                "Authorities for next session must be available at first block of current session",
            )
    }
}

fn is_first_block<B: Block>(num: NumberFor<B>, period: SessionPeriod) -> Option<SessionId> {
    let session = session_id_from_block_num::<B>(num, period);

    if first_block_of_session::<B>(session, period) == num {
        Some(session)
    } else {
        None
    }
}

pub struct SessionMapUpdater<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    B: Block,
    BE: Backend<B> + 'static,
{
    session_map: SharedSessionMap,
    client: Arc<C>,
    _phantom: PhantomData<(B, BE)>,
}

impl<C, B, BE> SessionMapUpdater<C, B, BE>
where
    C: ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    B: Block,
    BE: Backend<B> + 'static,
{
    pub fn new(client: Arc<C>) -> Self {
        Self {
            session_map: SharedSessionMap::new(),
            client,
            _phantom: PhantomData,
        }
    }

    pub fn session_map(&self) -> ReadOnlySessionMap {
        self.session_map.read_only()
    }

    async fn handle_first_block_of_session(&mut self, num: NumberFor<B>, session_id: SessionId) {
        debug!(target: "aleph-session-updater", "Handling first block #{:?} of session {:?}", num, session_id.0);
        let api_ref = self.client.runtime_api();
        let next_session = SessionId(session_id.0 + 1);
        self.session_map
            .update(
                next_session,
                get_authorities_for_session::<_, C>(api_ref, next_session, num),
            )
            .await;

        if session_id.0 == 0 {
            let api_ref = self.client.runtime_api();
            self.session_map
                .update(
                    session_id,
                    get_authorities_for_session::<_, C>(api_ref, session_id, num),
                )
                .await;
        }

        if session_id.0 >= 10 && session_id.0 % 10 == 0 {
            self.session_map
                .prune_below(SessionId(session_id.0 - 10))
                .await;
        }
    }

    pub async fn run(mut self, period: SessionPeriod) {
        let mut notifications = self.client.finality_notification_stream();

        // lets catch up
        for block_num in 0..=self.client.info().finalized_number.saturated_into::<u32>() {
            let block_num = block_num.saturated_into();
            if let Some(session_id) = is_first_block::<B>(block_num, period) {
                self.handle_first_block_of_session(block_num, session_id)
                    .await;
            }
        }

        while let Some(FinalityNotification { header, .. }) = notifications.next().await {
            let last_finalized = header.number();

            if let Some(session_id) = is_first_block::<B>(*last_finalized, period) {
                self.handle_first_block_of_session(*last_finalized, session_id)
                    .await;
            }
        }
    }
}
