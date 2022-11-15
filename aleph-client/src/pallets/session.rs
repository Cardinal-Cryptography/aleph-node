use primitives::SessionIndex;
use sp_core::H256;

use crate::{
    api, api::runtime_types::aleph_runtime::SessionKeys, AccountId, Connection, SignedConnection,
    TxStatus,
};

#[async_trait::async_trait]
pub trait SessionApi {
    async fn get_next_session_keys(
        &self,
        account: AccountId,
        at: Option<H256>,
    ) -> Option<SessionKeys>;
    async fn get_session(&self, at: Option<H256>) -> SessionIndex;
    async fn get_validators(&self, at: Option<H256>) -> Vec<AccountId>;
}

#[async_trait::async_trait]
pub trait SessionUserApi {
    async fn set_keys(&self, new_keys: SessionKeys, status: TxStatus) -> anyhow::Result<H256>;
}

#[async_trait::async_trait]
impl SessionApi for Connection {
    async fn get_next_session_keys(
        &self,
        account: AccountId,
        at: Option<H256>,
    ) -> Option<SessionKeys> {
        let addrs = api::storage().session().next_keys(account);

        self.get_storage_entry_maybe(&addrs, at).await
    }

    async fn get_session(&self, at: Option<H256>) -> SessionIndex {
        let addrs = api::storage().session().current_index();

        self.get_storage_entry_maybe(&addrs, at)
            .await
            .unwrap_or_default()
    }

    async fn get_validators(&self, at: Option<H256>) -> Vec<AccountId> {
        let addrs = api::storage().session().validators();

        self.get_storage_entry(&addrs, at).await
    }
}

#[async_trait::async_trait]
impl SessionUserApi for SignedConnection {
    async fn set_keys(&self, new_keys: SessionKeys, status: TxStatus) -> anyhow::Result<H256> {
        let tx = api::tx().session().set_keys(new_keys, vec![]);

        self.send_tx(tx, status).await
    }
}
