use std::{default::Default, marker::PhantomData, path::PathBuf, sync::Arc, time::Duration};

use futures_timer::Delay;
use log::{debug, error, info, trace, warn};
use tokio::{task::spawn_blocking, time::sleep};

use crate::{
    party::{
        authority::SubtaskCommon as AuthoritySubtaskCommon,
        task::{Handle, Task},
        traits::{AlephClient, Block, NodeSessionManager, RequestBlock, SessionInfo},
    },
    session_map::ReadOnlySessionMap,
    SessionId,
};

pub(crate) mod aggregator;
pub(crate) mod authority;
pub(crate) mod backup;
pub(crate) mod chain_tracker;
pub(crate) mod data_store;
pub mod impls;
pub(crate) mod member;
pub(crate) mod task;
pub mod traits;

pub(crate) struct ConsensusPartyParams<B: Block, RB, AC, AS, SI> {
    pub session_authorities: ReadOnlySessionMap,
    pub a_client: Arc<AC>,
    pub block_requester: RB,
    pub backup_saving_path: Option<PathBuf>,
    pub authority_tasks: AS,
    pub session_info: SI,
    pub _phantom: PhantomData<B>,
}

pub(crate) struct ConsensusParty<B, RB, AC, AS, SI>
where
    B: Block,
    RB: RequestBlock<B> + 'static,
    AC: AlephClient<B>,
    AS: NodeSessionManager,
    SI: SessionInfo<B>,
{
    session_authorities: ReadOnlySessionMap,
    a_client: Arc<AC>,
    block_requester: RB,
    backup_saving_path: Option<PathBuf>,
    authority_tasks: AS,
    session_info: SI,
    _phantom: PhantomData<B>,
}

const SESSION_STATUS_CHECK_PERIOD: Duration = Duration::from_millis(1000);

impl<B, RB, AC, AS, SI> ConsensusParty<B, RB, AC, AS, SI>
where
    B: Block,
    RB: RequestBlock<B> + 'static,
    AC: AlephClient<B>,
    AS: NodeSessionManager,
    SI: SessionInfo<B>,
{
    pub(crate) fn new(params: ConsensusPartyParams<B, RB, AC, AS, SI>) -> Self {
        let ConsensusPartyParams {
            session_authorities,
            block_requester,
            backup_saving_path,
            a_client,
            authority_tasks,
            session_info,
            ..
        } = params;
        Self {
            block_requester,
            session_authorities,
            backup_saving_path,
            a_client,
            authority_tasks,
            session_info,
            _phantom: PhantomData,
        }
    }

    async fn run_session(&mut self, session_id: SessionId) {
        let last_block = self.session_info.last_block_of_session(session_id);
        if let Some(previous_session_id) = session_id.0.checked_sub(1) {
            let backup_saving_path = self.backup_saving_path.clone();
            spawn_blocking(move || backup::remove(backup_saving_path, previous_session_id));
        }

        // Early skip attempt -- this will trigger during catching up (initial sync).
        if self.a_client.best_block_number() >= last_block {
            // We need to give the JustificationHandler some time to pick up the keychain for the new session,
            // validate justifications and finalize blocks. We wait 2000ms in total, checking every 200ms
            // if the last block has been finalized.
            for attempt in 0..10 {
                // We don't wait before the first attempt.
                if attempt != 0 {
                    Delay::new(Duration::from_millis(200)).await;
                }
                let last_finalized_number = self.a_client.finalized_number();
                if last_finalized_number >= last_block {
                    debug!(target: "aleph-party", "Skipping session {:?} early because block {:?} is already finalized", session_id, last_finalized_number);
                    return;
                }
            }
        }

        // We need to wait until session authority data is available for current session.
        // This should only be needed for the first ever session as all other session are known
        // at least one session earlier.
        let authority_data = match self
            .session_authorities
            .subscribe_to_insertion(session_id)
            .await
            .await
        {
            Err(e) => panic!(
                "Error while receiving the notification about current session {:?}",
                e
            ),
            Ok(authority_data) => authority_data,
        };
        let authorities = authority_data.authorities();

        trace!(target: "aleph-party", "Authority data for session {:?}: {:?}", session_id, authorities);
        let mut maybe_authority_task = if let Some(node_id) =
            self.authority_tasks.node_idx(authorities).await
        {
            match backup::rotate(self.backup_saving_path.clone(), session_id.0) {
                Ok(backup) => {
                    debug!(target: "aleph-party", "Running session {:?} as authority id {:?}", session_id, node_id);
                    Some(
                        self.authority_tasks
                            .spawn_authority_task_for_session(
                                session_id,
                                node_id,
                                backup,
                                authorities,
                            )
                            .await,
                    )
                }
                Err(err) => {
                    error!(
                        target: "AlephBFT-member",
                        "Error setting up backup saving for session {:?}. Not running the session: {}",
                        session_id, err
                    );
                    return;
                }
            }
        } else {
            debug!(target: "aleph-party", "Running session {:?} as non-authority", session_id);
            if let Err(e) = self
                .authority_tasks
                .start_nonvalidator_session(session_id, authorities)
            {
                warn!(target: "aleph-party", "Failed to start nonvalidator session{:?}:{:?}", session_id, e);
            }
            None
        };
        let mut check_session_status = Delay::new(SESSION_STATUS_CHECK_PERIOD);
        let next_session_id = SessionId(session_id.0 + 1);
        let mut start_next_session_network = Some(
            self.session_authorities
                .subscribe_to_insertion(next_session_id)
                .await,
        );
        loop {
            tokio::select! {
                _ = &mut check_session_status => {
                    let last_finalized_number = self.a_client.finalized_number();
                    if last_finalized_number >= last_block {
                        debug!(target: "aleph-party", "Terminating session {:?}", session_id);
                        break;
                    }
                    check_session_status = Delay::new(SESSION_STATUS_CHECK_PERIOD);
                },
                Some(next_session_authority_data) = async {
                    match &mut start_next_session_network {
                        Some(notification) => {
                            match notification.await {
                                Err(e) => {
                                    warn!(target: "aleph-party", "Error with subscription {:?}", e);
                                    start_next_session_network = Some(self.session_authorities.subscribe_to_insertion(next_session_id).await);
                                    None
                                },
                                Ok(next_session_authority_data) => {
                                    Some(next_session_authority_data)
                                }
                            }
                        },
                        None => None,
                    }
                } => {
                    let next_session_authorities = next_session_authority_data.authorities();
                    match self.authority_tasks.node_idx(next_session_authorities).await {
                         Some(_) => if let Err(e) = self
                                .authority_tasks
                                .early_start_validator_session(
                                    next_session_id,
                                    next_session_authorities,
                                ).await
                            {
                                warn!(target: "aleph-party", "Failed to early start validator session{:?}:{:?}", next_session_id, e);
                            }
                        None => {
                            if let Err(e) = self
                                .authority_tasks
                                .start_nonvalidator_session(next_session_id, next_session_authorities)
                            {
                                warn!(target: "aleph-party", "Failed to early start nonvalidator session{:?}:{:?}", next_session_id, e);
                            }
                        }
                    }
                    start_next_session_network = None;
                },
                Some(_) = async {
                    match maybe_authority_task.as_mut() {
                        Some(task) => Some(task.stopped().await),
                        None => None,
                    } } => {
                    warn!(target: "aleph-party", "Authority task ended prematurely, giving up for this session.");
                    maybe_authority_task = None;
                },
            }
        }
        if let Some(task) = maybe_authority_task {
            debug!(target: "aleph-party", "Stopping the authority task.");
            task.stop().await;
        }
        if let Err(e) = self.authority_tasks.stop_session(session_id) {
            warn!(target: "aleph-party", "Session Manager failed to stop in session {:?}: {:?}", session_id, e)
        }
    }

    pub async fn run(mut self) {
        let starting_session = self.catch_up().await;
        for curr_id in starting_session.0.. {
            info!(target: "aleph-party", "Running session {:?}.", curr_id);
            self.run_session(SessionId(curr_id)).await;
        }
    }

    async fn catch_up(&mut self) -> SessionId {
        let mut finalized_number = self.a_client.finalized_number();
        let mut previous_finalized_number = None;
        while self.block_requester.is_major_syncing()
            && Some(finalized_number) != previous_finalized_number
        {
            sleep(Duration::from_millis(500)).await;
            previous_finalized_number = Some(finalized_number);
            finalized_number = self.a_client.finalized_number();
        }
        self.session_info
            .session_id_from_block_num(finalized_number)
    }
}

// TODO: :(
#[cfg(test)]
mod tests {}
