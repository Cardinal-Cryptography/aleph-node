//! This module contains basic treasury actions testing. However, since currently we need to disable
//! proposing to treasury on Testnet, `treasury_access` test must have been simplified and thus
//! some part of this module is unused (`dead_code`). As soon as proposals are enabled once again,
//! we should recover original scenario.

use codec::{Compact, Decode};
use log::info;
use sp_core::Pair;
use sp_runtime::{AccountId32, MultiAddress};
use std::{thread, thread::sleep, time::Duration};
use substrate_api_client::{
    compose_extrinsic, AccountId, Balance, ExtrinsicParams, GenericAddress, XtStatus,
};

use aleph_client::{
    balances_transfer, get_free_balance, get_tx_fee_info, send_xt, staking_treasury_payout,
    total_issuance, treasury_account, treasury_proposals_counter, wait_for_event, AnyConnection,
    Extrinsic, RootConnection, SignedConnection,
};

use crate::{accounts::get_validators_keys, config::Config, transfer::setup_for_tipped_transfer};

pub fn channeling_fee_and_tip(config: &Config) -> anyhow::Result<()> {
    let tip = 10_000u128;
    let (connection, to) = setup_for_tipped_transfer(config, tip);
    let treasury = treasury_account();

    let possibly_treasury_gain_from_staking = staking_treasury_payout(&connection);
    let treasury_balance_before = get_free_balance(&connection, &treasury);
    let issuance_before = total_issuance(&connection);
    info!(
        "[+] Treasury balance before tx: {}. Total issuance: {}.",
        treasury_balance_before, issuance_before
    );

    let tx = balances_transfer(&connection, &to, 1000u128, XtStatus::Finalized);
    let treasury_balance_after = get_free_balance(&connection, &treasury);
    let issuance_after = total_issuance(&connection);
    check_treasury_issuance(
        possibly_treasury_gain_from_staking,
        treasury_balance_after,
        issuance_before,
        issuance_after,
    );

    let fee_info = get_tx_fee_info(&connection, &tx);
    let fee = fee_info.fee_without_weight + fee_info.adjusted_weight;
    check_treasury_balance(
        possibly_treasury_gain_from_staking,
        treasury_balance_before,
        treasury_balance_after,
        fee,
        tip,
    );

    Ok(())
}

fn check_treasury_issuance(
    possibly_treasury_gain_from_staking: u128,
    treasury_balance_after: Balance,
    issuance_before: u128,
    issuance_after: u128,
) {
    info!(
        "[+] Treasury balance after tx: {}. Total issuance: {}.",
        treasury_balance_after, issuance_after
    );

    if issuance_after > issuance_before {
        let difference = issuance_after - issuance_before;
        assert_eq!(
            difference % possibly_treasury_gain_from_staking,
            0,
            "Unexpectedly {} was minted, and it's not related to staking treasury reward which is {}",
            difference, possibly_treasury_gain_from_staking
        );
    }

    assert!(
        issuance_before <= issuance_after,
        "Unexpectedly {} was burned",
        issuance_before - issuance_after
    );
}

fn check_treasury_balance(
    possibly_treasury_gain_from_staking: u128,
    treasury_balance_before: Balance,
    treasury_balance_after: Balance,
    fee: Balance,
    tip: Balance,
) {
    let treasury_balance_diff = treasury_balance_after - (treasury_balance_before + fee + tip);
    assert_eq!(
        treasury_balance_diff % possibly_treasury_gain_from_staking,
        0,
        "Incorrect amount was channeled to the treasury: before = {}, after = {}, fee = {}, tip = \
        {}. We can be different only as multiples of staking treasury reward {}, but the remainder \
        is {}",
        treasury_balance_before,
        treasury_balance_after,
        fee,
        tip,
        possibly_treasury_gain_from_staking,
        treasury_balance_diff % possibly_treasury_gain_from_staking,
    );
}

pub fn treasury_access(config: &Config) -> anyhow::Result<()> {
    let proposer = get_validators_keys(config)[0].clone();
    let beneficiary = AccountId::from(proposer.public());
    let connection = SignedConnection::new(&config.node, proposer);

    let proposals_counter_before = treasury_proposals_counter(&connection);
    propose_treasury_spend(10u128, &beneficiary, &connection);
    let proposals_counter_after = treasury_proposals_counter(&connection);
    assert_eq!(
        proposals_counter_before, proposals_counter_after,
        "Proposal was created: deposit was not high enough"
    );

    Ok(())
}

type ProposalTransaction = Extrinsic<([u8; 2], Compact<u128>, MultiAddress<AccountId, ()>)>;
fn propose_treasury_spend(
    value: u128,
    beneficiary: &AccountId32,
    connection: &SignedConnection,
) -> ProposalTransaction {
    let xt = compose_extrinsic!(
        connection.as_connection(),
        "Treasury",
        "propose_spend",
        Compact(value),
        GenericAddress::Id(beneficiary.clone())
    );
    send_xt(
        connection,
        xt.clone(),
        Some("treasury spend"),
        XtStatus::Finalized,
    );
    xt
}

#[allow(dead_code)]
type GovernanceTransaction = Extrinsic<([u8; 2], Compact<u32>)>;

#[allow(dead_code)]
fn send_treasury_approval(proposal_id: u32, connection: &RootConnection) -> GovernanceTransaction {
    let xt = compose_extrinsic!(
        connection.as_connection(),
        "Treasury",
        "approve_proposal",
        Compact(proposal_id)
    );
    send_xt(
        connection,
        xt.clone(),
        Some("treasury approval"),
        XtStatus::Finalized,
    );
    xt
}

#[allow(dead_code)]
fn treasury_approve(proposal_id: u32, connection: &RootConnection) -> anyhow::Result<()> {
    send_treasury_approval(proposal_id, connection);
    wait_for_approval(connection, proposal_id)
}

#[allow(dead_code)]
fn send_treasury_rejection(proposal_id: u32, connection: &RootConnection) -> GovernanceTransaction {
    let xt = compose_extrinsic!(
        connection.as_connection(),
        "Treasury",
        "reject_proposal",
        Compact(proposal_id)
    );
    send_xt(
        connection,
        xt.clone(),
        Some("treasury rejection"),
        XtStatus::Finalized,
    );
    xt
}

#[allow(dead_code)]
fn treasury_reject(proposal_id: u32, connection: &RootConnection) -> anyhow::Result<()> {
    let (c, p) = (connection.clone(), proposal_id);
    let listener = thread::spawn(move || wait_for_rejection(&c, p));
    send_treasury_rejection(proposal_id, connection);
    listener.join().unwrap()
}

#[allow(dead_code)]
fn wait_for_approval<C: AnyConnection>(connection: &C, proposal_id: u32) -> anyhow::Result<()> {
    loop {
        let approvals: Vec<u32> = connection
            .as_connection()
            .get_storage_value("Treasury", "Approvals", None)
            .unwrap()
            .unwrap();
        if approvals.contains(&proposal_id) {
            info!("[+] Proposal {:?} approved successfully", proposal_id);
            return Ok(());
        } else {
            info!(
                "[+] Still waiting for approval for proposal {:?}",
                proposal_id
            );
            sleep(Duration::from_millis(500))
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Decode, Copy, Clone)]
struct ProposalRejectedEvent {
    proposal_id: u32,
    _slashed: u128,
}

#[allow(dead_code)]
fn wait_for_rejection<C: AnyConnection>(connection: &C, proposal_id: u32) -> anyhow::Result<()> {
    wait_for_event(
        connection,
        ("Treasury", "Rejected"),
        |e: ProposalRejectedEvent| {
            info!("[+] Rejected proposal {:?}", e.proposal_id);
            proposal_id.eq(&e.proposal_id)
        },
    )
    .map(|_| ())
}
