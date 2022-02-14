use codec::Compact;
use log::info;

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sp_core::Pair;
use substrate_api_client::compose_call;
use substrate_api_client::{compose_extrinsic, GenericAddress, XtStatus};

use crate::accounts::accounts_from_seeds;
use crate::config::Config;
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
    address: &str,
    signer_account: KeyPair,
    stashes_accounts_key_pairs: &[KeyPair],
    endowment: u128,
) {
    let connection = create_connection(address).set_signer(signer_account);
    let batch_endow: Vec<_> = stashes_accounts_key_pairs
        .iter()
        .map(|stash| {
            compose_call!(
                connection.metadata,
                "Balances",
                "transfer",
                GenericAddress::Id(AccountId::from(stash.public())),
                Compact(endowment)
            )
        })
        .collect();

    let extrinsic = compose_extrinsic!(connection, "Utility", "batch", batch_endow);

    let block_hash = connection
        .send_extrinsic(extrinsic.hex_encode(), XtStatus::InBlock)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");
    info!(
        "[+] A batch of endow account transactions was included in {} block.",
        block_hash
    );
}
fn bond(address: &str, initial_stake: u128, controller: &KeyPair) {
    let connection = create_connection(address).set_signer(controller.clone());
    let account_id = AccountId::from(controller.public());
    let bond_extrinsic = connection.staking_bond(
        GenericAddress::Id(account_id.clone()),
        initial_stake,
        RewardDestination::Staked,
    );
    let block_hash = connection
        .send_extrinsic(bond_extrinsic.hex_encode(), XtStatus::InBlock)
        .expect("Could not send extrinsic")
        .expect("Could not get tx hash");
    info!(
        "[+] Controller account {} was bond to stash account {} in block {}.",
        account_id, account_id, block_hash
    );
}
fn validate(address: &str, controller: &KeyPair) {
    let controller_account_id = AccountId::from(controller.public());
    let connection = create_connection(address).set_signer(controller.clone());
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

fn nominate(address: &str, nominator_key_pair: &KeyPair, nominee_key_pair: &KeyPair) {
    let nominator_account_id = AccountId::from(nominator_key_pair.public());
    let nominee_account_id = AccountId::from(nominee_key_pair.public());
    let connection = create_connection(address).set_signer(nominator_key_pair.clone());

    let nominate_extrinsic =
        connection.staking_nominate(vec![GenericAddress::Id(nominee_account_id.clone())]);
    let block_hash = connection
        .send_extrinsic(nominate_extrinsic.hex_encode(), XtStatus::InBlock)
        .expect("Could not send extrinsic")
        .expect("Could not get tx hash");
    info!(
        "[+] Nominator account {} nominated account {} in block {}.",
        nominator_account_id, nominee_account_id, block_hash
    );
}

fn get_top_finalized_block<F>(
    address: &str,
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
    address: &str,
    sender_account_key_pair: KeyPair,
) -> anyhow::Result<u32> {
    let current_finalized_block =
        get_top_finalized_block(address, sender_account_key_pair.clone(), |_| true)?;
    // TODO: this should be expressed in terms of runtime chain configuration, not hardcoded values
    // 30 = block_in_session(10) * sessions_in_era (3)
    const BLOCKS_PER_ERA: u32 = 30;

    let next_full_era_block_number =
        ((current_finalized_block.number + BLOCKS_PER_ERA) / BLOCKS_PER_ERA + 1) * BLOCKS_PER_ERA;
    info!(
        "[+] Top finalized block is {}, waiting for block {}",
        current_finalized_block.number, next_full_era_block_number
    );
    let _ = get_top_finalized_block(address, sender_account_key_pair, |header| {
        header.number >= next_full_era_block_number
    })?;

    let current_era = next_full_era_block_number / BLOCKS_PER_ERA;
    Ok(current_era)
}

fn payout_staker(
    address: &str,
    sender_account_key_pair: KeyPair,
    stash_account_key_pair: &KeyPair,
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
    let validator_stashes = validators
        .iter()
        .map(|v| String::from(v) + "//stash")
        .collect();
    let validator_accounts_key_pairs = accounts_from_seeds(Some(validators).as_ref());
    let stashes_accounts_key_pairs = accounts_from_seeds(Some(validator_stashes).as_ref());

    (stashes_accounts_key_pairs, validator_accounts_key_pairs)
}

// 1. endow stash accounts balances, controller accounts are already endowed in chainspec
// 2. bond controller account to stash account, stash = controller and set controller to StakerStatus::Validate
// 3. bond controller account to stash account, stash = controller and set controller to StakerStatus::Nominate
// 4. wait for new era
// 5. send payout stakers tx
pub fn staking_test(config: &Config) -> anyhow::Result<()> {
    const TOKEN: u128 = 1_000_000_000_000;
    const VALIDATOR_STAKE: u128 = 25_000 * TOKEN;
    const NOMINATOR_STAKE: u128 = 1_000 * TOKEN;

    let node = &config.node;
    let (stashes_accounts, validator_accounts) = get_key_pairs();

    endow_stash_balances(
        node,
        validator_accounts[0].clone(),
        &stashes_accounts,
        VALIDATOR_STAKE,
    );

    validator_accounts.par_iter().for_each(|account| {
        bond(node, VALIDATOR_STAKE, account);
    });

    validator_accounts
        .par_iter()
        .for_each(|account| validate(node, account));

    stashes_accounts
        .par_iter()
        .for_each(|nominator| bond(node, NOMINATOR_STAKE, nominator));

    stashes_accounts
        .par_iter()
        .zip(validator_accounts.par_iter())
        .for_each(|(nominator, nominee)| nominate(node, nominator, nominee));

    let sender = validator_accounts[0].clone();
    let current_era = wait_for_full_reward_era(node, sender.clone())?;
    info!(
        "[+] Full era {} passed, claiming rewards for era {}",
        current_era,
        current_era - 1
    );

    for validator_account in validator_accounts {
        payout_staker(node, sender.clone(), &validator_account, current_era - 1);
    }

    Ok(())
}
