use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use aleph_primitives::SessionAuthorityData;
use async_trait::async_trait;
use sp_runtime::testing::UintAuthorityId;

use crate::{
    oneshot,
    party::{
        backup::ABFTBackup,
        manager::AuthorityTask,
        traits::{Block, ChainState, NodeSessionManager, SessionInfo, SyncState},
        ConsensusParty, ConsensusPartyParams,
    },
    session_map::SharedSessionMap,
    AuthorityId, NodeIndex, SessionId,
};

type AMutex<T> = Arc<Mutex<T>>;

pub struct SimpleBlock;

impl Block for SimpleBlock {
    type Number = u32;
    type Hash = String;
}

#[derive(Clone, Debug)]
pub struct MockChainState {
    pub best_block: AMutex<u32>,
    pub finalized_block: AMutex<u32>,
}

impl MockChainState {
    pub fn new() -> Self {
        Self {
            best_block: Arc::new(Mutex::new(0)),
            finalized_block: Arc::new(Mutex::new(0)),
        }
    }

    pub fn set_best_block(&self, best_block: u32) {
        *self.best_block.lock().unwrap() = best_block;
    }

    pub fn set_finalized_block(&self, finalized_block: u32) {
        *self.best_block.lock().unwrap() = finalized_block;
    }
}

#[derive(Clone, Debug)]
pub struct MockSyncState {
    pub is_syncing: AMutex<bool>,
}

impl MockSyncState {
    pub fn new() -> Self {
        Self {
            is_syncing: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_is_syncing(&self, is_syncing: bool) {
        *self.is_syncing.lock().unwrap() = is_syncing;
    }
}

#[derive(Clone, Debug)]
pub struct MockNodeSessionManager {
    pub nonvalidator_session_started: AMutex<HashSet<SessionId>>,
    pub validator_session_started: AMutex<HashSet<SessionId>>,
    pub session_stopped: AMutex<HashSet<SessionId>>,
    pub session_early_started: AMutex<HashSet<SessionId>>,
    pub node_id: AMutex<Option<NodeIndex>>,
}

impl MockNodeSessionManager {
    pub fn new() -> Self {
        Self {
            nonvalidator_session_started: Arc::new(Mutex::new(Default::default())),
            validator_session_started: Arc::new(Mutex::new(Default::default())),
            session_stopped: Arc::new(Mutex::new(Default::default())),
            session_early_started: Arc::new(Mutex::new(Default::default())),
            node_id: Arc::new(Mutex::new(Default::default())),
        }
    }

    pub fn set_node_id(&self, node_id: Option<NodeIndex>) {
        *self.node_id.lock().unwrap() = node_id;
    }
}

pub struct MockSessionInfo {
    pub session_period: u32,
}

impl MockSessionInfo {
    pub fn new() -> Self {
        Self { session_period: 90 }
    }
}

impl ChainState<SimpleBlock> for Arc<MockChainState> {
    fn best_block_number(&self) -> u32 {
        *self.best_block.lock().unwrap()
    }

    fn finalized_number(&self) -> u32 {
        *self.finalized_block.lock().unwrap()
    }
}

impl SyncState<SimpleBlock> for Arc<MockSyncState> {
    fn is_major_syncing(&self) -> bool {
        *self.is_syncing.lock().unwrap()
    }
}

#[async_trait]
impl NodeSessionManager for Arc<MockNodeSessionManager> {
    type Error = ();

    async fn spawn_authority_task_for_session(
        &self,
        session: SessionId,
        _node_id: NodeIndex,
        _backup: ABFTBackup,
        _authorities: &[AuthorityId],
    ) -> AuthorityTask {
        let mut x = self.validator_session_started.lock().unwrap();
        x.insert(session);

        let (exit, _) = oneshot::channel();
        let handle = async { Ok(()) };

        AuthorityTask::new(Box::pin(handle), NodeIndex(0), exit)
    }

    async fn early_start_validator_session(
        &self,
        session: SessionId,
        _authorities: &[AuthorityId],
    ) -> Result<(), Self::Error> {
        let mut x = self.session_early_started.lock().unwrap();

        x.insert(session);

        Ok(())
    }

    fn start_nonvalidator_session(
        &self,
        session: SessionId,
        _authorities: &[AuthorityId],
    ) -> Result<(), Self::Error> {
        let mut x = self.nonvalidator_session_started.lock().unwrap();

        x.insert(session);

        Ok(())
    }

    fn stop_session(&self, session: SessionId) -> Result<(), Self::Error> {
        let mut x = self.session_stopped.lock().unwrap();

        x.insert(session);

        Ok(())
    }

    async fn node_idx(&self, _authorities: &[AuthorityId]) -> Option<NodeIndex> {
        *self.node_id.lock().unwrap()
    }
}

impl SessionInfo<SimpleBlock> for MockSessionInfo {
    fn session_id_from_block_num(&self, n: u32) -> SessionId {
        SessionId(n / self.session_period)
    }

    fn last_block_of_session(&self, session_id: SessionId) -> u32 {
        (session_id.0 + 1) * self.session_period - 1
    }

    fn first_block_of_session(&self, session_id: SessionId) -> u32 {
        session_id.0 * self.session_period
    }
}
