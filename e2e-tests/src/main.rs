use std::env;
use std::iter;
use std::time::Instant;

use clap::Parser;
use common::create_connection;
use log::info;
use sp_core::crypto::Ss58Codec;
use sp_core::Pair;
use substrate_api_client::{compose_call, compose_extrinsic, AccountId, XtStatus};

use config::Config;

use crate::transfer::{test_fee_calculation, test_token_transfer};
use crate::treasury::{test_channeling_fee, test_treasury_access};
use crate::utils::*;
use crate::waiting::{wait_for_finalized_block, wait_for_session};

mod config;
mod transfer;
mod treasury;
mod utils;
mod waiting;

fn main() -> anyhow::Result<()> {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "warn");
    }
    env_logger::init();

    let config: Config = Config::parse();

    run(test_finalization, "finalization", config.clone())?;
    run(test_fee_calculation, "fee calculation", config.clone())?;
    run(test_token_transfer, "token transfer", config.clone())?;
    run(test_channeling_fee, "channeling fee", config.clone())?;
    run(test_treasury_access, "treasury access", config.clone())?;
    run(test_change_validators, "validators change", config)?;

    Ok(())
}

fn run<T>(
    testcase: fn(Config) -> anyhow::Result<T>,
    name: &str,
    config: Config,
) -> anyhow::Result<()> {
    println!("Running test: {}", name);
    let start = Instant::now();
    testcase(config).map(|_| {
        let elapsed = Instant::now().duration_since(start);
        println!("Ok! Elapsed time {}ms", elapsed.as_millis());
    })
}

fn test_finalization(config: Config) -> anyhow::Result<u32> {
    let connection = create_connection(config.node);
    wait_for_finalized_block(&connection, 1)
}

fn test_change_validators(config: Config) -> anyhow::Result<()> {
    let Config { node, seeds, .. } = config.clone();

    let accounts = accounts(seeds);
    let sudo = get_sudo(config);

    let connection = create_connection(node).set_signer(sudo);

    let validators_before: Vec<AccountId> = connection
        .get_storage_value("Session", "Validators", None)?
        .unwrap();

    info!("[+] Validators before tx: {:#?}", validators_before);

    let new_validators: Vec<AccountId> = accounts
        .into_iter()
        .map(|pair| pair.public().into())
        .chain(iter::once(
            AccountId::from_ss58check("5EHkv1FCd4jeQmVrbYhrETL1EAr8NJxNbukDRT4FaYWbjW8f").unwrap(),
        ))
        .collect();

    info!("[+] New validators {:#?}", new_validators);

    // wait beyond session 1
    let current_session_index = wait_for_session(&connection, 1)?;
    let session_for_change = current_session_index + 2;
    info!("[+] Current session index {:?}", current_session_index);

    let call = compose_call!(
        connection.metadata,
        "Aleph",
        "change_validators",
        new_validators.clone(),
        session_for_change
    );

    let tx = compose_extrinsic!(connection, "Sudo", "sudo_unchecked_weight", call, 0_u64);

    // send and watch extrinsic until finalized
    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");

    info!("[+] change_validators transaction hash: {}", tx_hash);

    // wait for the change to be applied
    wait_for_session(&connection, session_for_change)?;

    let validators_after: Vec<AccountId> = connection
        .get_storage_value("Session", "Validators", None)?
        .unwrap();

    info!("[+] Validators after tx: {:#?}", validators_after);

    assert!(new_validators.eq(&validators_after));

    Ok(())
}
