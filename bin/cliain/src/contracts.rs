use crate::{Command, ContractMessageTranscoder};
use aleph_client::{send_xt, wait_for_event, AnyConnection, SignedConnection};
use anyhow::anyhow;
use codec::{Compact, Decode};
use contract_metadata::ContractMetadata;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use sp_core::{Pair, H256};
use std::{
    fs::{self, File},
    path::Path,
};
use substrate_api_client::{compose_extrinsic, AccountId, GenericAddress, XtStatus};

#[derive(Debug, Decode, Clone)]
pub struct ContractCodeRemovedEvent {
    code_hash: H256,
}

#[derive(Debug, Decode, Clone)]
pub struct ContractInstantiatedEvent {
    deployer: AccountId,
    contract: AccountId,
}

#[derive(Debug, Decode, Clone)]
pub struct ContractCodeStoredEvent {
    code_hash: H256,
}

#[derive(Debug, Decode, Clone, Serialize, Deserialize)]
pub struct InstantiateWithCodeReturnValue {
    pub contract: AccountId,
    pub code_hash: H256,
}

fn storage_deposit(storage_deposit_limit: Option<u128>) -> Option<Compact<u128>> {
    storage_deposit_limit.map(Compact)
}

pub fn upload_code(
    signed_connection: SignedConnection,
    command: Command,
) -> anyhow::Result<ContractCodeStoredEvent> {
    if let Command::ContractUploadCode {
        wasm_path,
        storage_deposit_limit,
    } = command
    {
        let connection = signed_connection.as_connection();

        let wasm = fs::read(wasm_path).expect("WASM artifact not found");
        debug!(target: "contracts", "Found WASM contract code {:?}", wasm);

        let xt = compose_extrinsic!(
            connection,
            "Contracts",
            "upload_code",
            wasm, // code
            storage_deposit(storage_deposit_limit)
        );

        debug!(target: "contracts", "Prepared `upload_code` extrinsic {:?}", xt);

        let block_hash = send_xt(&connection, xt, Some("upload_code"), XtStatus::InBlock);

        debug!(target: "contracts", "instantiate_with_code extrinsic included in block {:#?}", block_hash);

        let code_stored_event: ContractCodeStoredEvent = wait_for_event(
            &connection,
            ("Contracts", "CodeStored"),
            |e: ContractCodeStoredEvent| {
                info!(target : "contracts", "Received CodeStored event {:?}", e);
                true
            },
        )?;

        Ok(code_stored_event)
    } else {
        panic!("should never get here")
    }
}

pub fn instantiate(
    signed_connection: SignedConnection,
    command: Command,
) -> anyhow::Result<ContractInstantiatedEvent> {
    if let Command::ContractInstantiate {
        balance,
        gas_limit,
        storage_deposit_limit,
        code_hash,
        metadata_path,
        constructor,
        args,
    } = command
    {
        let connection = signed_connection.as_connection();

        let metadata = load_metadata(&metadata_path)?;
        let transcoder = ContractMessageTranscoder::new(&metadata);
        let data = transcoder.encode(&constructor, &args.unwrap_or_default())?;

        debug!("Encoded constructor data {:?}", data);

        let xt = compose_extrinsic!(
            connection,
            "Contracts",
            "instantiate",
            Compact(balance),
            Compact(gas_limit),
            storage_deposit(storage_deposit_limit),
            code_hash,
            data,             // The input data to pass to the contract constructor
            Vec::<u8>::new()  // salt used for the address derivation
        );

        debug!(target: "contracts", "Prepared `instantiate` extrinsic {:?}", xt);

        let _block_hash = send_xt(&connection, xt, Some("instantiate"), XtStatus::InBlock);

        let contract_instantiated_event: ContractInstantiatedEvent = wait_for_event(
            &connection,
            ("Contracts", "Instantiated"),
            |e: ContractInstantiatedEvent| {
                info!(target : "contracts", "Received ContractInstantiated event {:?}", e);
                match &connection.signer {
                    Some(signer) => AccountId::from(signer.public()).eq(&e.deployer),
                    None => panic!("Should never get here"),
                }
            },
        )?;

        Ok(contract_instantiated_event)
    } else {
        panic!("should never get here")
    }
}

pub fn instantiate_with_code(
    signed_connection: SignedConnection,
    command: Command,
) -> anyhow::Result<InstantiateWithCodeReturnValue> {
    if let Command::ContractInstantiateWithCode {
        wasm_path,
        metadata_path,
        constructor,
        args,
        balance,
        gas_limit,
        storage_deposit_limit,
    } = command
    {
        let connection = signed_connection.as_connection();

        let wasm = fs::read(wasm_path).expect("WASM artifact not found");
        debug!(target: "contracts", "Found WASM contract code {:?}", wasm);

        let metadata = load_metadata(&metadata_path)?;
        let transcoder = ContractMessageTranscoder::new(&metadata);
        let data = transcoder.encode(&constructor, &args.unwrap_or_default())?;

        debug!("Encoded constructor data {:?}", data);

        let xt = compose_extrinsic!(
            connection,
            "Contracts",
            "instantiate_with_code",
            Compact(balance),
            Compact(gas_limit),
            storage_deposit(storage_deposit_limit),
            wasm,             // code
            data,             // The input data to pass to the contract constructor
            Vec::<u8>::new()  // salt used for the address derivation
        );

        debug!(target: "contracts", "Prepared `instantiate_with_code` extrinsic {:?}", xt);

        let _block_hash = send_xt(
            &connection,
            xt,
            Some("instantiate_with_code"),
            XtStatus::InBlock,
        );

        let code_stored_event: ContractCodeStoredEvent = wait_for_event(
            &connection,
            ("Contracts", "CodeStored"),
            |e: ContractCodeStoredEvent| {
                info!(target : "contracts", "Received CodeStored event {:?}", e);
                // TODO : can we pre-calculate what the code hash will be?
                true
            },
        )?;

        let contract_instantiated_event: ContractInstantiatedEvent = wait_for_event(
            &connection,
            ("Contracts", "Instantiated"),
            |e: ContractInstantiatedEvent| {
                info!(target : "contracts", "Received ContractInstantiated event {:?}", e);
                match &connection.signer {
                    Some(signer) => AccountId::from(signer.public()).eq(&e.deployer),
                    None => panic!("Should never get here"),
                }
            },
        )?;

        Ok(InstantiateWithCodeReturnValue {
            contract: contract_instantiated_event.contract,
            code_hash: code_stored_event.code_hash,
        })
    } else {
        panic!("should never get here")
    }
}

pub fn call(signed_connection: SignedConnection, command: Command) -> anyhow::Result<()> {
    if let Command::ContractCall {
        destination,
        balance,
        gas_limit,
        storage_deposit_limit,
        message,
        args,
        metadata_path,
    } = command
    {
        let connection = signed_connection.as_connection();

        let metadata = load_metadata(&metadata_path)?;
        let transcoder = ContractMessageTranscoder::new(&metadata);
        let data = transcoder.encode(&message, &args.unwrap_or_default())?;

        debug!("Encoded call data {:?}", data);

        let xt = compose_extrinsic!(
            connection,
            "Contracts",
            "call",
            GenericAddress::Id(destination),
            Compact(balance),
            Compact(gas_limit),
            storage_deposit(storage_deposit_limit),
            data // The input data to pass to the contract message
        );

        debug!(target: "contracts", "Prepared `call` extrinsic {:?}", xt);

        let block_hash = send_xt(&connection, xt, Some("call"), XtStatus::Finalized);

        info!(target: "contracts", "contract call extrinsic finalized in block {:#?}", block_hash);
        Ok(())
    } else {
        panic!("should never get here")
    }
}

pub fn remove_code(
    signed_connection: SignedConnection,
    command: Command,
) -> anyhow::Result<ContractCodeRemovedEvent> {
    if let Command::ContractRemoveCode { code_hash } = command {
        let connection = signed_connection.as_connection();

        let xt = compose_extrinsic!(connection, "Contracts", "remove_code", code_hash);

        debug!(target: "contracts", "Prepared `remove_code` extrinsic {:?}", xt);

        let _block_hash = send_xt(&connection, xt, Some("remove_code"), XtStatus::InBlock);

        let contract_removed_event: ContractCodeRemovedEvent = wait_for_event(
            &connection,
            ("Contracts", "CodeRemoved"),
            |e: ContractCodeRemovedEvent| {
                info!(target : "contracts", "Received ContractCodeRemoved event {:?}", e);
                e.code_hash.eq(&code_hash)
            },
        )?;

        Ok(contract_removed_event)
    } else {
        panic!("should never get here")
    }
}

fn load_metadata(path: &Path) -> anyhow::Result<ink_metadata::InkProject> {
    let file = File::open(&path).expect("Failed to open metadata file");
    let metadata: ContractMetadata =
        serde_json::from_reader(file).expect("Failed to deserialize metadata file");
    let ink_metadata = serde_json::from_value(serde_json::Value::Object(metadata.abi))
        .expect("Failed to deserialize ink project metadata");

    if let ink_metadata::MetadataVersioned::V3(ink_project) = ink_metadata {
        Ok(ink_project)
    } else {
        Err(anyhow!("Unsupported ink metadata version. Expected V3"))
    }
}
