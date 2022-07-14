use std::fmt::Debug;

use async_trait::async_trait;
use sp_runtime::traits::{Block as BlockT, NumberFor};

use crate::{
    network,
    party::{authority::Task as AuthorityTask, backup::ABFTBackup},
    AuthorityId, NodeIndex, SessionId,
};

pub trait Block {
    type Number: Debug + PartialOrd + Copy;
    type Hash: Debug;
}

impl<T> Block for T
where
    T: BlockT,
{
    type Number = NumberFor<T>;
    type Hash = <T as BlockT>::Hash;
}

#[async_trait]
pub trait AlephClient<B: Block> {
    fn best_block_number(&self) -> <B as Block>::Number;
    fn finalized_number(&self) -> <B as Block>::Number;
}

#[async_trait]
pub trait NodeSessionManager {
    type Error: Debug;

    async fn spawn_authority_task_for_session(
        &self,
        session: SessionId,
        node_id: NodeIndex,
        backup: ABFTBackup,
        authorithies: Vec<AuthorityId>,
    ) -> AuthorityTask;

    async fn early_start_validator_session(
        &self,
        session: SessionId,
        authorities: Vec<AuthorityId>,
    ) -> Result<(), Self::Error>;
    fn start_nonvalidator_session(
        &self,
        session: SessionId,
        authorities: Vec<AuthorityId>,
    ) -> Result<(), Self::Error>;
    fn stop_session(&self, session: SessionId) -> Result<(), Self::Error>;
    async fn node_idx(&self, authorities: &[AuthorityId]) -> Option<NodeIndex>;
}

pub trait RequestBlock<B: Block> {
    fn is_major_syncing(&self) -> bool;
}

impl<B: BlockT, RB> RequestBlock<B> for RB
where
    RB: network::RequestBlocks<B>,
{
    fn is_major_syncing(&self) -> bool {
        self.is_major_syncing()
    }
}

pub trait SessionInfo<B: Block> {
    fn session_id_from_block_num(&self, n: B::Number) -> SessionId;
    fn last_block_of_session(&self, session_id: SessionId) -> B::Number;
    fn first_block_of_session(&self, session_id: SessionId) -> B::Number;
}
