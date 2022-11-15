use codec::{Compact, Decode};
use primitives::Balance;
use serde::Serialize;
use subxt::{
    ext::{sp_core::H256, sp_runtime::MultiAddress},
    rpc_params,
};

use crate::{
    api, pallet_contracts::wasm::OwnerInfo, AccountId, Connection, SignedConnection, TxStatus,
};

#[derive(Serialize)]
pub struct ContractCallArgs {
    pub origin: AccountId,
    pub dest: AccountId,
    pub value: Balance,
    pub gas_limit: u64,
    pub input_data: Vec<u8>,
}

#[async_trait::async_trait]
pub trait ContractsApi {
    async fn get_owner_info(&self, code_hash: H256, at: Option<H256>) -> Option<OwnerInfo>;
}

#[async_trait::async_trait]
pub trait ContractsUserApi {
    async fn upload_code(
        &self,
        code: Vec<u8>,
        storage_limit: Option<Compact<u128>>,
        status: TxStatus,
    ) -> anyhow::Result<H256>;
    async fn instantiate(
        &self,
        code_hash: H256,
        balance: Balance,
        gas_limit: u64,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<H256>;
    async fn instantiate_with_code(
        &self,
        code: Vec<u8>,
        balance: Balance,
        gas_limit: u64,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<H256>;
    async fn call(
        &self,
        destination: AccountId,
        balance: Balance,
        gas_limit: u64,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<H256>;
    async fn remove_code(&self, code_hash: H256, status: TxStatus) -> anyhow::Result<H256>;
}

#[async_trait::async_trait]
pub trait ContractRpc {
    async fn call_and_get<T: Decode>(&self, args: ContractCallArgs) -> anyhow::Result<T>;
}

#[async_trait::async_trait]
impl ContractsApi for Connection {
    async fn get_owner_info(&self, code_hash: H256, at: Option<H256>) -> Option<OwnerInfo> {
        let addrs = api::storage().contracts().owner_info_of(code_hash);

        self.get_storage_entry_maybe(&addrs, at).await
    }
}

#[async_trait::async_trait]
impl ContractsUserApi for SignedConnection {
    async fn upload_code(
        &self,
        code: Vec<u8>,
        storage_limit: Option<Compact<u128>>,
        status: TxStatus,
    ) -> anyhow::Result<H256> {
        let tx = api::tx().contracts().upload_code(code, storage_limit);

        self.send_tx(tx, status).await
    }

    async fn instantiate(
        &self,
        code_hash: H256,
        balance: Balance,
        gas_limit: u64,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<H256> {
        let tx = api::tx().contracts().instantiate(
            balance,
            gas_limit,
            storage_limit,
            code_hash,
            data,
            salt,
        );

        self.send_tx(tx, status).await
    }

    async fn instantiate_with_code(
        &self,
        code: Vec<u8>,
        balance: Balance,
        gas_limit: u64,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<H256> {
        let tx = api::tx().contracts().instantiate_with_code(
            balance,
            gas_limit,
            storage_limit,
            code,
            data,
            salt,
        );

        self.send_tx(tx, status).await
    }

    async fn call(
        &self,
        destination: AccountId,
        balance: Balance,
        gas_limit: u64,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<H256> {
        let tx = api::tx().contracts().call(
            MultiAddress::Id(destination),
            balance,
            gas_limit,
            storage_limit,
            data,
        );
        self.send_tx(tx, status).await
    }

    async fn remove_code(&self, code_hash: H256, status: TxStatus) -> anyhow::Result<H256> {
        let tx = api::tx().contracts().remove_code(code_hash);

        self.send_tx(tx, status).await
    }
}

#[async_trait::async_trait]
impl ContractRpc for Connection {
    async fn call_and_get<T: Decode>(&self, args: ContractCallArgs) -> anyhow::Result<T> {
        let params = rpc_params![args];

        self.rpc_call("contracts_call".to_string(), params).await
    }
}
