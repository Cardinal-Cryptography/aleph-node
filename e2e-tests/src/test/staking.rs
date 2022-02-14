use log::info;

use sp_core::Pair;
use substrate_api_client::{compose_extrinsic, GenericAddress, XtStatus};

use crate::accounts::accounts_from_seeds;
use crate::config::Config;
use crate::transfer::transfer;
use crate::Header;
use crate::KeyPair;
use anyhow::anyhow;
use common::create_connection;
use pallet_staking::ValidatorPrefs;
use sp_runtime::Perbill;
use std::sync::mpsc::channel;
use substrate_api_client::extrinsic::staking::RewardDestination;
use substrate_api_client::AccountId;

fn endow_stash_balances(
    address: String,
    signer_account: KeyPair,
    stashes_accounts_key_pairs: &Vec<KeyPair>,
    endowment: u128,
) {
    let connection = create_connection(address).set_signer(signer_account);
    for stash_account_key_pair in stashes_accounts_key_pairs {
        let to = AccountId::from(stash_account_key_pair.public());
        transfer(&to, endowment, &connection, XtStatus::InBlock);
    }
}

fn bond(address: String, initial_stake: u128, controller: KeyPair) {
    let controller_account_id = AccountId::from(controller.public());
    let connection = create_connection(address).set_signer(controller);

    let bond_extrinsic = connection.staking_bond(
        GenericAddress::Id(controller_account_id.clone()),
        initial_stake,
        RewardDestination::Staked,
    );
    let finalized_block_hash = connection
        .send_extrinsic(bond_extrinsic.hex_encode(), XtStatus::InBlock)
        .expect("Could not send extrinsic")
        .expect("Could not get tx hash");
    info!(
        "[+] Controller account {} was bond to stash account {} in finalized {} block.",
        controller_account_id, controller_account_id, finalized_block_hash
    );
}

fn validate(address: String, controller_key_pair: KeyPair) {
    let controller_account_id = AccountId::from(controller_key_pair.public());
    let connection = create_connection(address).set_signer(controller_key_pair);

    crate::send_extrinsic!(
        connection,
        "Staking",
        "validate",
        XtStatus::InBlock,
        |tx_hash| info!(
            "[+] Controller {} intended itself to be elected in the next era, tx: {}",
            controller_account_id, tx_hash
        ),
        ValidatorPrefs {
            blocked: false,
            commission: Perbill::from_percent(10),
        }
    );
}

fn nominate(address: String, nominator_key_pair: KeyPair, nominee_key_pair: KeyPair) {
    let nominator_account_id = AccountId::from(nominator_key_pair.public());
    let nominee_account_id = AccountId::from(nominee_key_pair.public());
    let connection = create_connection(address).set_signer(nominator_key_pair);

    let nominate_extrinsic =
        connection.staking_nominate(vec![GenericAddress::Id(nominee_account_id.clone())]);
    let finalized_block_hash = connection
        .send_extrinsic(nominate_extrinsic.hex_encode(), XtStatus::InBlock)
        .expect("Could not send extrinsic")
        .expect("Could not get tx hash");
    info!(
        "[+] Nominator account {} nominated account {} in finalized {} block.",
        nominator_account_id, nominee_account_id, finalized_block_hash
    );
}

fn get_top_finalized_block<F>(
    address: String,
    sender_account_key_pair: KeyPair,
    predicate: F,
) -> anyhow::Result<Header>
where
    F: Fn(&Header) -> bool,
{
    let connection = create_connection(address).set_signer(sender_account_key_pair);
    let (sender, receiver) = channel();
    connection.subscribe_finalized_heads(sender)?;

    while let Ok(header) = receiver
        .recv()
        .map(|h| serde_json::from_str::<Header>(&h).unwrap())
    {
        if predicate(&header) {
            return Ok(header);
        }
    }
    Err(anyhow!("Failed to receive finalized head block"))
}

fn wait_for_full_reward_era(
    address: String,
    sender_account_key_pair: KeyPair,
) -> anyhow::Result<u32> {
    let current_finalized_block =
        get_top_finalized_block(address.clone(), sender_account_key_pair.clone(), |_| true)?;
    // TODO: this should be expressed in terms of runtime chain configuration, not hardcoded values
    // 30 = block_in_session(10) * sessions_in_era (3)
    const BLOCKS_PER_ERA: u32 = 30;

    let next_full_era_block_number =
        ((current_finalized_block.number + BLOCKS_PER_ERA) / BLOCKS_PER_ERA + 1) * BLOCKS_PER_ERA;
    info!(
        "[+] Top finalized block is {}, waiting for block {}",
        current_finalized_block.number, next_full_era_block_number
    );
    let _ = get_top_finalized_block(address.clone(), sender_account_key_pair.clone(), |header| {
        header.number >= next_full_era_block_number
    })?;

    let current_era = next_full_era_block_number / BLOCKS_PER_ERA;
    Ok(current_era)
}

fn payout_staker(
    address: String,
    sender_account_key_pair: KeyPair,
    stash_account_key_pair: KeyPair,
    era_number: u32,
) {
    let stash_account_id = AccountId::from(stash_account_key_pair.public());
    let connection = create_connection(address).set_signer(sender_account_key_pair);

    crate::send_extrinsic!(
        connection,
        "Staking",
        "payout_stakers",
        XtStatus::Finalized,
        |tx_hash| info!(
            "[+] Payout for staker {} done in era {}, tx: {}",
            stash_account_id, era_number, tx_hash
        ),
        stash_account_id.clone(),
        era_number
    );
}

fn get_key_pairs() -> (Vec<KeyPair>, Vec<KeyPair>) {
    let validators = vec![
        String::from("//Damian"),
        String::from("//Hansu"),
        String::from("//Tomasz"),
        String::from("//Zbyszko"),
    ];
    let validator_stashes: Vec<_> = validators
        .iter()
        .map(|v| String::from(v) + "//stash")
        .collect();
    let validator_accounts_key_pairs = accounts_from_seeds(Some(validators));
    let stashes_accounts_key_pairs = accounts_from_seeds(Some(validator_stashes));

    (stashes_accounts_key_pairs, validator_accounts_key_pairs)
}

pub fn staking_test(config: Config) -> anyhow::Result<()> {
    const TOKEN: u128 = 1_000_000_000_000;
    const VALIDATOR_STAKE: u128 = 25_000u128 * TOKEN;
    const NOMINATOR_STAKE: u128 = 1_000u128 * TOKEN;

    let Config { node, .. } = config.clone();
    let (stashes_accounts, validator_accounts) = get_key_pairs();
    endow_stash_balances(
        node.clone(),
        validator_accounts[0].clone(),
        &stashes_accounts,
        VALIDATOR_STAKE,
    );

    for validator_account in validator_accounts.clone() {
        let validator_account = validator_account.to_owned();
        bond(node.clone(), VALIDATOR_STAKE, validator_account.clone());
        validate(node.clone(), validator_account);
    }

    for (nominator, nominee) in stashes_accounts
        .into_iter()
        .zip(validator_accounts.clone().into_iter())
    {
        let nominator = nominator.to_owned();
        let nominee = nominee.to_owned();
        bond(node.clone(), NOMINATOR_STAKE, nominator.clone());
        nominate(node.clone(), nominator, nominee);
    }

    let sender = validator_accounts[0].clone();
    let current_era = wait_for_full_reward_era(node.clone(), sender.clone())?;
    info!(
        "[+] Full era {} passed, claiming rewards for era {}",
        current_era,
        current_era - 1
    );

    for validator_account in validator_accounts {
        let validator_account = validator_account.to_owned();
        payout_staker(
            node.clone(),
            sender.clone(),
            validator_account,
            current_era - 1,
        );
    }

    Ok(())
}
