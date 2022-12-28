use log::info;
use primitives::{BlockNumber, EraIndex, SessionIndex};

use crate::{
    pallets::{elections::ElectionsApi, staking::StakingApi},
    BlockHash, Connection,
};

/// Any object that implements retrieving various info about blocks.
#[async_trait::async_trait]
pub trait BlocksApi {
    /// Returns the first block of a session.
    /// * `session` - number of the session to query the first block from
    async fn first_block_of_session(&self, session: SessionIndex) -> Option<BlockHash>;

    /// Returns hash of a given block.
    /// * `block` - number of the block
    async fn get_block_hash(&self, block: BlockNumber) -> Option<BlockHash>;

    /// Returns the most recent block from the current best chain.
    async fn get_best_block(&self) -> BlockNumber;

    /// Returns the most recent block from the finalized chain.
    async fn get_finalized_block_hash(&self) -> BlockHash;

    /// Returns number of a given block hash.
    /// * `block` - hash of the block to query its number
    async fn get_block_number(&self, block: BlockHash) -> Option<BlockNumber>;
}

/// Any object that implements interaction logic between session and staking.
#[async_trait::async_trait]
pub trait SessionEraApi {
    /// Returns which era given session is.
    /// * `session` - session index
    async fn get_active_era_for_session(&self, session: SessionIndex) -> EraIndex;
}

#[async_trait::async_trait]
impl BlocksApi for Connection {
    async fn first_block_of_session(&self, session: SessionIndex) -> Option<BlockHash> {
        let period = self.get_session_period().await;
        let block_num = period * session;

        self.get_block_hash(block_num).await
    }

    async fn get_block_hash(&self, block: BlockNumber) -> Option<BlockHash> {
        info!(target: "aleph-client", "querying block hash for number #{}", block);
        self.client
            .rpc()
            .block_hash(Some(block.into()))
            .await
            .unwrap()
    }

    async fn get_best_block(&self) -> BlockNumber {
        self.client
            .rpc()
            .header(None)
            .await
            .unwrap()
            .unwrap()
            .number
    }

    async fn get_finalized_block_hash(&self) -> BlockHash {
        self.client.rpc().finalized_head().await.unwrap()
    }

    async fn get_block_number(&self, block: BlockHash) -> Option<BlockNumber> {
        self.client
            .rpc()
            .header(Some(block))
            .await
            .unwrap()
            .map(|h| h.number)
    }
}

#[async_trait::async_trait]
impl SessionEraApi for Connection {
    async fn get_active_era_for_session(&self, session: SessionIndex) -> EraIndex {
        let block = self.first_block_of_session(session).await;
        self.get_active_era(block).await
    }
}
