//! Contains types and functions simplifying common contract-related operations.
//!
//! For example, you could write this wrapper around (some of) the functionality of openbrush PSP22
//! contracts using the building blocks provided by this module:
//!
//! ```no_run
//! # use anyhow::{Result, Context};
//! # use aleph_client::AccountId;
//! # use aleph_client::{Connection, SignedConnection};
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
//!     fn transfer(&self, conn: &SignedConnection, to: AccountId, amount: u128) -> Result<()> {
//!         self.contract.contract_exec(
//!             conn,
//!             "PSP22::transfer",
//!             vec![to.to_string().as_str(), amount.to_string().as_str(), "0x00"].as_slice(),
//!         )
//!     }
//!
//!     fn balance_of(&self, conn: &Connection, account: AccountId) -> Result<u128> {
//!         self.contract.contract_read(
//!             conn,
//!             "PSP22::balance_of",
//!             &vec![account.to_string().as_str()],
//!         )?.try_into()
//!     }
//! }
//! ```

mod convertible_value;
pub mod event;

use std::{
    fmt::{Debug, Formatter},
    fs::File,
};

use anyhow::{anyhow, Context, Result};
use contract_metadata::ContractMetadata;
use contract_transcode::ContractMessageTranscoder;
pub use convertible_value::ConvertibleValue;
use ink_metadata::{InkProject, MetadataVersioned};
use serde_json::{from_reader, from_str, from_value, json};
use substrate_api_client::{compose_extrinsic, GenericAddress, XtStatus};

use crate::{try_send_xt, AccountId, AnyConnection, SignedConnection};

/// Represents a contract instantiated on the chain.
pub struct ContractInstance {
    address: AccountId,
    ink_project: InkProject,
}

impl ContractInstance {
    const MAX_READ_GAS: u64 = 500000000000u64;
    const MAX_GAS: u64 = 100000000000u64;
    const PAYABLE_VALUE: u64 = 0u64;
    const STORAGE_FEE_LIMIT: Option<u128> = None;

    /// Creates a new contract instance under `address` with metadata read from `metadata_path`.
    pub fn new(address: AccountId, metadata_path: &str) -> Result<Self> {
        Ok(Self {
            address,
            ink_project: load_metadata(metadata_path)?,
        })
    }

    /// The address of this contract instance.
    pub fn address(&self) -> &AccountId {
        &self.address
    }

    /// The metadata of this contract instance.
    pub fn ink_project(&self) -> &InkProject {
        &self.ink_project
    }

    /// Reads the value of a read-only, 0-argument call via RPC.
    pub fn contract_read0<C: AnyConnection>(
        &self,
        conn: &C,
        message: &str,
    ) -> Result<ConvertibleValue> {
        self.contract_read(conn, message, &[])
    }

    /// Reads the value of a read-only call via RPC.
    pub fn contract_read<C: AnyConnection>(
        &self,
        conn: &C,
        message: &str,
        args: &[&str],
    ) -> Result<ConvertibleValue> {
        let payload = self.encode(message, args)?;
        let request = self.contract_read_request(&payload);
        let response = conn
            .as_connection()
            .get_request(request)?
            .context("RPC request error - there may be more info in node logs.")?;
        let response_data = from_str::<serde_json::Value>(&response)?;
        let hex_data = response_data["result"]["Ok"]["data"]
            .as_str()
            .context("Contract response data not found - the contract address might be invalid.")?;
        self.decode_response(message, hex_data)
    }

    /// Executes a 0-argument contract call.
    pub fn contract_exec0(&self, conn: &SignedConnection, message: &str) -> Result<()> {
        self.contract_exec(conn, message, &[])
    }

    /// Executes a contract call.
    pub fn contract_exec(
        &self,
        conn: &SignedConnection,
        message: &str,
        args: &[&str],
    ) -> Result<()> {
        let data = self.encode(message, args)?;
        let xt = compose_extrinsic!(
            conn.as_connection(),
            "Contracts",
            "call",
            GenericAddress::Id(self.address.clone()),
            Compact(Self::PAYABLE_VALUE),
            Compact(Self::MAX_GAS),
            Self::STORAGE_FEE_LIMIT,
            data
        );

        try_send_xt(conn, xt, Some("Contracts call"), XtStatus::InBlock)
            .context("Failed to exec contract message")?;
        Ok(())
    }

    fn contract_read_request(&self, payload: &[u8]) -> serde_json::Value {
        let payload = hex::encode(payload);
        json!({
            "jsonrpc": "2.0",
            "method": "contracts_call",
            "params": [{
                "origin": self.address,
                "dest": self.address,
                "value": 0,
                "gasLimit": Self::MAX_READ_GAS,
                "inputData": payload
            }],
            "id": 1
        })
    }

    fn encode(&self, message: &str, args: &[&str]) -> Result<Vec<u8>> {
        ContractMessageTranscoder::new(&self.ink_project).encode(message, args)
    }

    fn decode_response(&self, from: &str, contract_response: &str) -> Result<ConvertibleValue> {
        let contract_response = contract_response.trim_start_matches("0x");
        let bytes = hex::decode(contract_response)?;
        ContractMessageTranscoder::new(&self.ink_project)
            .decode_return(from, &mut bytes.as_slice())
            .map(ConvertibleValue)
    }
}

impl Debug for ContractInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractInstance")
            .field("address", &self.address)
            .field("ink_project", &self.ink_project)
            .finish()
    }
}

/// Helper for loading contract metadata from a file.
///
/// The contract-metadata lib contains a similar function starting with version 0.2. It seems that
/// version conflicts with some of our other dependencies, however, if we upgrade in the future we
/// can drop this function in favour of their implementation.
fn load_metadata(path: &str) -> Result<InkProject> {
    let file = File::open(path)?;
    let metadata: ContractMetadata = from_reader(file)?;
    let ink_metadata = from_value(serde_json::Value::Object(metadata.abi))?;

    if let MetadataVersioned::V3(ink_project) = ink_metadata {
        Ok(ink_project)
    } else {
        Err(anyhow!("Unsupported ink metadata version. Expected V3"))
    }
}
