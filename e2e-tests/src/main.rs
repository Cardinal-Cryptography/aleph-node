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

use crate::utils::*;
use crate::waiting::{wait_for_finalized_block, wait_for_session};

mod config;
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

fn test_fee_calculation(config: Config) -> anyhow::Result<()> {
    let (connection, from, to) = setup_for_transfer(config);

    let balance_before = get_free_balance(&from, &connection);
    info!("[+] Account {} balance before tx: {}", to, balance_before);

    let transfer_value = 1000u128;
    let tx = transfer(&to, transfer_value, &connection);

    let balance_after = get_free_balance(&from, &connection);
    info!("[+] Account {} balance after tx: {}", to, balance_after);

    let FeeInfo {
        fee_without_weight,
        unadjusted_weight,
        adjusted_weight,
    } = get_tx_fee_info(&connection, &tx);
    let multiplier = 1; // corresponds to `ConstantFeeMultiplierUpdate`
    assert_eq!(
        multiplier * unadjusted_weight,
        adjusted_weight,
        "Weight fee was adjusted incorrectly: raw fee = {}, adjusted fee = {}",
        unadjusted_weight,
        adjusted_weight
    );

    let expected_fee = fee_without_weight + adjusted_weight;
    assert_eq!(
        balance_before - transfer_value - expected_fee,
        balance_after,
        "Incorrect balance: before = {}, after = {}, tx = {}, expected fee = {}",
        balance_before,
        balance_after,
        transfer_value,
        expected_fee
    );

    Ok(())
}

fn test_token_transfer(config: Config) -> anyhow::Result<()> {
    let (connection, _, to) = setup_for_transfer(config);

    let balance_before = get_free_balance(&to, &connection);
    info!("[+] Account {} balance before tx: {}", to, balance_before);

    let transfer_value = 1000u128;
    transfer(&to, transfer_value, &connection);

    let balance_after = get_free_balance(&to, &connection);
    info!("[+] Account {} balance after tx: {}", to, balance_after);

    assert_eq!(
        balance_before + transfer_value,
        balance_after,
        "before = {}, after = {}, tx = {}",
        balance_before,
        balance_after,
        transfer_value
    );

    Ok(())
}

// todo: check channeling tips
fn test_channeling_fee(config: Config) -> anyhow::Result<()> {
    let (connection, _, to) = setup_for_transfer(config);
    let treasury = get_treasury_account(&connection);

    let treasury_balance_before = get_free_balance(&treasury, &connection);
    let issuance_before = get_total_issuance(&connection);
    info!(
        "[+] Treasury balance before tx: {}. Total issuance: {}.",
        treasury_balance_before, issuance_before
    );

    let tx = transfer(&to, 1000u128, &connection);

    let treasury_balance_after = get_free_balance(&treasury, &connection);
    let issuance_after = get_total_issuance(&connection);
    info!(
        "[+] Treasury balance after tx: {}. Total issuance: {}.",
        treasury_balance_after, issuance_after
    );

    assert!(
        issuance_after <= issuance_before,
        "Unexpectedly {} was minted",
        issuance_after - issuance_before
    );
    assert!(
        issuance_before <= issuance_after,
        "Unexpectedly {} was burned",
        issuance_before - issuance_after
    );

    let fee_info = get_tx_fee_info(&connection, &tx);
    let fee = fee_info.fee_without_weight + fee_info.adjusted_weight;

    assert_eq!(
        treasury_balance_before + fee,
        treasury_balance_after,
        "Incorrect amount was channeled to the treasury: before = {}, after = {}, fee = {}",
        treasury_balance_before,
        treasury_balance_after,
        fee
    );

    Ok(())
}

fn test_treasury_access(config: Config) -> anyhow::Result<()> {
    let Config { node, seeds, .. } = config.clone();

    let proposer = accounts(seeds)[0].to_owned();
    let beneficiary = AccountId::from(proposer.public());
    let connection = create_connection(node).set_signer(proposer);

    propose_treasury_spend(0u128, &beneficiary, &connection);
    propose_treasury_spend(0u128, &beneficiary, &connection);
    let proposals_counter = get_proposals_counter(&connection);
    assert!(proposals_counter >= 2, "Proposal was not created");

    let sudo = get_sudo(config);
    let connection = connection.set_signer(sudo);

    treasury_approve(proposals_counter - 2, &connection)?;
    treasury_reject(proposals_counter - 1, &connection)?;

    Ok(())
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
