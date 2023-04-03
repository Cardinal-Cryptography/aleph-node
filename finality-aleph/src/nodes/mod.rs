mod validator_node;

use aleph_primitives::BlockNumber;
use sp_runtime::traits::{Block, Header};
pub use validator_node::run_validator_node;

use crate::{
    justification::{SessionInfo, SessionInfoProvider},
    session::SessionBoundaryInfo,
    session_map::ReadOnlySessionMap,
    sync::SessionVerifier,
    HashNum,
};

#[cfg(test)]
pub mod testing {
    pub use super::validator_node::new_pen;
}

struct SessionInfoProviderImpl {
    session_authorities: ReadOnlySessionMap,
    session_info: SessionBoundaryInfo,
}

#[async_trait::async_trait]
impl<H> SessionInfoProvider<HashNum<H>, SessionVerifier> for SessionInfoProviderImpl
where
    H: Header<Number = BlockNumber>,
{
    async fn for_block_num(&self, number: BlockNumber) -> SessionInfo<HashNum<H>, SessionVerifier> {
        let current_session = self.session_info.session_id_from_block_num(number);
        let last_block_height = self.session_info.last_block_of_session(current_session);
        let verifier = self
            .session_authorities
            .get(current_session)
            .await
            .map(|authority_data| authority_data.into());

        SessionInfo::new(current_session, last_block_height, verifier)
    }
}
