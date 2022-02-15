use crate::{
    accounts::accounts_from_seeds, config::Config, BlockNumber, Connection, Header, KeyPair,
};
use anyhow::anyhow;
use codec::Compact;
use common::create_connection;
use log::info;
use pallet_staking::ValidatorPrefs;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use sp_core::Pair;
use sp_runtime::Perbill;
use std::sync::mpsc::channel;
use substrate_api_client::{
    compose_call, compose_extrinsic, extrinsic::staking::RewardDestination, AccountId,
    GenericAddress, XtStatus,
};

fn send_xt(connection: &Connection, xt: String, xt_name: &'static str) {
    let block_hash = connection
        .send_extrinsic(xt, XtStatus::InBlock)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");
    let block_number = connection
        .get_header::<Header>(Some(block_hash))
        .expect("Could not fetch header")
        .expect("Block exists; qed")
        .number;
    info!(
        "Transaction {} was included in block {}.",
        xt_name, block_number
    );
}

fn endow_stash_balances(connection: &Connection, keys: &[KeyPair], endowment: u128) {
    let batch_endow: Vec<_> = keys
        .iter()
        .map(|key| {
            compose_call!(
                connection.metadata,
                "Balances",
                "transfer",
                GenericAddress::Id(AccountId::from(key.public())),
                Compact(endowment)
            )
        })
        .collect();

    let xt = compose_extrinsic!(connection, "Utility", "batch", batch_endow);
    send_xt(connection, xt.hex_encode(), "batch of endow balances");
}

fn bond(address: &str, initial_stake: u128, controller: &KeyPair) {
    let connection = create_connection(address).set_signer(controller.clone());
    let account_id = GenericAddress::Id(AccountId::from(controller.public()));

    let xt = connection.staking_bond(account_id, initial_stake, RewardDestination::Staked);
    send_xt(&connection, xt.hex_encode(), "bond");
}

fn validate(address: &str, controller: &KeyPair) {
    let connection = create_connection(address).set_signer(controller.clone());
    let prefs = ValidatorPrefs {
        blocked: false,
        commission: Perbill::from_percent(10),
    };

    let xt = compose_extrinsic!(connection, "Staking", "validate", prefs);
    send_xt(&connection, xt.hex_encode(), "validate");
}

fn nominate(address: &str, nominator_key_pair: &KeyPair, nominee_key_pair: &KeyPair) {
    let nominee_account_id = AccountId::from(nominee_key_pair.public());
    let connection = create_connection(address).set_signer(nominator_key_pair.clone());

    let xt = connection.staking_nominate(vec![GenericAddress::Id(nominee_account_id)]);
    send_xt(&connection, xt.hex_encode(), "nominate");
}

fn payout_stakers(address: &str, validator: KeyPair, era_number: BlockNumber) {
    let account = AccountId::from(validator.public());
    let connection = create_connection(address).set_signer(validator);
    let xt = compose_extrinsic!(connection, "Staking", "payout_stakers", account, era_number);

    send_xt(&connection, xt.hex_encode(), "payout_stakers");
}

fn top_finalized_block_number<F>(
    connection: &Connection,
    predicate: F,
) -> anyhow::Result<BlockNumber>
where
    F: Fn(BlockNumber) -> bool,
{
    let (sender, receiver) = channel();
    connection.subscribe_finalized_heads(sender)?;

    while let Ok(number) = receiver
        .recv()
        .map(|h| serde_json::from_str::<Header>(&h).unwrap().number)
    {
        if predicate(number) {
            return Ok(number);
        }
    }
    Err(anyhow!("Failed to receive finalized head block"))
}

fn wait_for_full_reward_era(connection: &Connection) -> anyhow::Result<BlockNumber> {
    let current_finalized_block_number = top_finalized_block_number(connection, |_| true)?;
    let session_period: u32 = connection
        .get_storage_value("Elections", "SessionPeriod", None)
        .unwrap()
        .unwrap();
    let sessions_per_era: u32 = connection
        .get_storage_value("Elections", "SessionsPerEra", None)
        .unwrap()
        .unwrap();
    let blocks_per_era = session_period * sessions_per_era;

    let next_full_era_block_number =
        ((current_finalized_block_number + blocks_per_era) / blocks_per_era + 1) * blocks_per_era;
    info!(
        "Top finalized block is {}, waiting for block {}",
        current_finalized_block_number, next_full_era_block_number
    );
    let _ = top_finalized_block_number(connection, |number| number >= next_full_era_block_number)?;

    let current_era = next_full_era_block_number / blocks_per_era;
    Ok(current_era)
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

    let (stashes_accounts, validator_accounts) = get_key_pairs();

    let node = &config.node;
    let sender = validator_accounts[0].clone();
    let connection = create_connection(node).set_signer(sender);

    endow_stash_balances(&connection, &stashes_accounts, VALIDATOR_STAKE);

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

    let current_era = wait_for_full_reward_era(&connection)?;
    info!(
        "Full era {} passed, claiming rewards for era {}",
        current_era,
        current_era - 1
    );

    validator_accounts
        .into_par_iter()
        .for_each(|account| payout_stakers(node, account, current_era - 1));

    Ok(())
}
