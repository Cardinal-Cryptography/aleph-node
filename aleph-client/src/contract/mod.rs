//! Contains types and functions simplifying common contract-related operations.
//!
//! For example, you could write this wrapper around (some of) the functionality of PSP22
//! contracts using the building blocks provided by this module:
//!
//! ```no_run
//! # use anyhow::{Result, Context};
//! # use aleph_client::{AccountId, Balance};
//! # use aleph_client::{Connection, SignedConnection, TxInfo};
//! # use aleph_client::contract::ContractInstance;
//! #
//! #[derive(Debug)]
//! struct PSP22TokenInstance {
//!     contract: ContractInstance,
//! }
//!
//! impl PSP22TokenInstance {
//!     fn new(address: AccountId, metadata_path: &Option<String>) -> Result<Self> {
//!         let metadata_path = metadata_path
//!             .as_ref()
//!             .context("PSP22Token metadata not set.")?;
//!         Ok(Self {
//!             contract: ContractInstance::new(address, metadata_path)?,
//!         })
//!     }
//!
//!     async fn transfer(&self, conn: &SignedConnection, to: AccountId, amount: Balance) -> Result<TxInfo> {
//!         self.contract.exec_api().exec(
//!             conn,
//!             "PSP22::transfer",
//!             vec![to.to_string().as_str(), amount.to_string().as_str(), "0x00"].as_slice(),
//!         ).await
//!     }
//!
//!     async fn balance_of(&self, conn: &Connection, account: AccountId) -> Result<Balance> {
//!         self.contract.read_api().read(
//!             conn,
//!             "PSP22::balance_of",
//!             &vec![account.to_string().as_str()],
//!         ).await?
//!     }
//! }
//! ```

mod convertible_value;
pub mod event;

use std::fmt::{Debug, Formatter};

use anyhow::{anyhow, Context, Result};
use contract_transcode::ContractMessageTranscoder;
pub use convertible_value::ConvertibleValue;
use log::info;
use pallet_contracts_primitives::ContractExecResult;
use serde::__private::Clone;

use crate::{
    connections::TxInfo,
    contract_transcode::Value,
    pallets::contract::{ContractCallArgs, ContractRpc, ContractsUserApi, EventRecord},
    sp_weights::weight_v2::Weight,
    AccountId, Balance, BlockHash, ConnectionApi, SignedConnectionApi, TxStatus,
};

/// Represents a contract instantiated on the chain.
pub struct ContractInstance {
    address: AccountId,
    transcoder: ContractMessageTranscoder,
}

/// Builder for read only contract call
pub struct ReadonlyContractCallBuilder<'a> {
    instance: &'a ContractInstance,
    at: Option<BlockHash>,
    sender: AccountId,
}

impl<'a> ReadonlyContractCallBuilder<'a> {
    /// Sets the block hash to execute the call at. If not set, by default the latest block is used.
    pub fn at(&mut self, at: BlockHash) -> &mut Self {
        self.at = Some(at);
        self
    }

    /// Overriders `sender` of the contract call as if it was executed by them. If not set,
    /// by default the contract address is used.
    pub fn override_sender(&mut self, sender: AccountId) -> &mut Self {
        self.sender = sender;
        self
    }

    /// Reads the value of a read-only, 0-argument call via RPC.
    pub async fn read0<T: TryFrom<ConvertibleValue, Error = anyhow::Error>, C: ConnectionApi>(
        &self,
        conn: &C,
        message: &str,
    ) -> Result<T> {
        self.read::<String, T, C>(conn, message, &[]).await
    }

    /// Reads the value of a read-only call via RPC.
    pub async fn read<
        S: AsRef<str> + Debug,
        T: TryFrom<ConvertibleValue, Error = anyhow::Error>,
        C: ConnectionApi,
    >(
        &self,
        conn: &C,
        message: &str,
        args: &[S],
    ) -> Result<T> {
        let result = self
            .instance
            .dry_run(conn, message, args, self.sender.clone(), 0, self.at)
            .await?
            .result
            .map_err(|e| anyhow!("Contract exec failed {:?}", e))?;

        let decoded = self.instance.decode(message, result.data)?;
        ConvertibleValue(decoded).try_into()?
    }
}

/// Builder for a contract call that will be submitted to chain
pub struct ExecCallBuilder<'a> {
    instance: &'a ContractInstance,
    value: Balance,
    max_gas: Option<u64>,
    max_proof_size: Option<u64>,
}

impl<'a> ExecCallBuilder<'a> {
    /// Sets the `value` balance to send with the call.
    pub fn value(&mut self, value: Balance) -> &mut Self {
        self.value = value;
        self
    }

    /// Sets the `ref_time` parameter of `gas_limit` in the call.
    pub fn max_gas_override(&mut self, max_gas_override: u64) -> &mut Self {
        self.max_gas = Some(max_gas_override);
        self
    }

    /// Sets the `proof_size` parameter of `gas_limit` in the call.
    pub fn max_proof_size_override(&mut self, max_proof_size_override: u64) -> &mut Self {
        self.max_proof_size = Some(max_proof_size_override);
        self
    }

    /// Executes a 0-argument contract call sending the given amount of value with it.
    pub async fn exec0<C: SignedConnectionApi>(&self, conn: &C, message: &str) -> Result<TxInfo> {
        self.exec::<C, String>(conn, message, &[]).await
    }

    /// Executes a contract call sending the given amount of value with it.
    pub async fn exec<C: SignedConnectionApi, S: AsRef<str> + Debug>(
        &self,
        conn: &C,
        message: &str,
        args: &[S],
    ) -> Result<TxInfo> {
        let dry_run_result = self
            .instance
            .dry_run(
                conn,
                message,
                args,
                conn.account_id().clone(),
                self.value,
                None,
            )
            .await?;

        let data = self.instance.encode(message, args)?;
        conn.call(
            self.instance.address.clone(),
            self.value,
            Weight {
                ref_time: self
                    .max_gas
                    .unwrap_or(dry_run_result.gas_required.ref_time()),
                proof_size: self
                    .max_proof_size
                    .unwrap_or(dry_run_result.gas_required.proof_size()),
            },
            None,
            data,
            TxStatus::Finalized,
        )
        .await
    }
}

impl ContractInstance {
    /// Creates a new contract instance under `address` with metadata read from `metadata_path`.
    pub fn new(address: AccountId, metadata_path: &str) -> Result<Self> {
        Ok(Self {
            address,
            transcoder: ContractMessageTranscoder::load(metadata_path)?,
        })
    }

    /// The address of this contract instance.
    pub fn address(&self) -> &AccountId {
        &self.address
    }

    /// Returns read-only contract call builder. By default, it will use latest block's state
    /// and contract's self address as a sender.
    pub fn read_api(&self) -> ReadonlyContractCallBuilder {
        ReadonlyContractCallBuilder {
            instance: self,
            at: None,
            sender: self.address.clone(),
        }
    }

    /// Returns a builder for a contract call that will be submitted to chain. By default, it sends:
    /// - zero `value` amount with the call,
    /// - `gas_limit` is equal to the one calculated during the dry-run,
    pub fn exec_api(&self) -> ExecCallBuilder {
        ExecCallBuilder {
            instance: self,
            value: 0,
            max_gas: None,
            max_proof_size: None,
        }
    }

    async fn dry_run<S: AsRef<str> + Debug, C: ConnectionApi>(
        &self,
        conn: &C,
        message: &str,
        args: &[S],
        sender: AccountId,
        value: Balance,
        at: Option<BlockHash>,
    ) -> Result<ContractExecResult<Balance, EventRecord>> {
        let payload = self.encode(message, args)?;
        let args = ContractCallArgs {
            origin: sender,
            dest: self.address.clone(),
            value,
            gas_limit: None,
            input_data: payload,
            storage_deposit_limit: None,
        };

        let contract_read_result = conn
            .call_and_get(args, at)
            .await
            .context("RPC request error - there may be more info in node logs.")?;

        if !contract_read_result.debug_message.is_empty() {
            info!(
                target: "aleph_client::contract",
                "Dry-run debug messages: {:?}",
                core::str::from_utf8(&contract_read_result.debug_message)
                    .unwrap_or("<Invalid UTF8>")
                    .split('\n')
                    .filter(|m| !m.is_empty())
                    .collect::<Vec<_>>()
            );
        }

        // For dry run, failed transactions don't return `Err` but `Ok(_)`
        // and we have to inspect flags manually.
        if let Ok(res) = &contract_read_result.result {
            if res.did_revert() {
                return Err(anyhow!(
                    "Dry-run call reverted, decoded result: {:?}",
                    self.decode(message, res.data.clone())
                ));
            }
        }

        Ok(contract_read_result)
    }

    fn encode<S: AsRef<str> + Debug>(&self, message: &str, args: &[S]) -> Result<Vec<u8>> {
        self.transcoder.encode(message, args)
    }

    fn decode(&self, message: &str, data: Vec<u8>) -> Result<Value> {
        self.transcoder.decode_return(message, &mut data.as_slice())
    }
}

impl Debug for ContractInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractInstance")
            .field("address", &self.address)
            .finish()
    }
}
