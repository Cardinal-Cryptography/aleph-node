use aleph_client::{
    account_from_keypair, balances_batch_transfer, balances_transfer, change_validators,
    get_block_hash, get_era, get_session, get_session_period, get_sessions_per_era, send_xt,
    wait_for_full_era_completion, wait_for_next_era, AnyConnection, KeyPair, SignedConnection,
};
use log::info;
use primitives::{staking::MIN_VALIDATOR_BOND, Balance, SessionIndex, TOKEN};
use substrate_api_client::{AccountId, XtStatus};

use crate::{
    accounts::{accounts_seeds_to_keys, get_validators_keys, get_validators_seeds},
    rewards::{check_points, reset_validator_keys, set_invalid_keys_for_validator},
    Config,
};

fn get_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[0..2].to_vec()
}

fn get_non_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[2..].to_vec()
}

fn get_non_reserved_members_for_session(config: &Config, session: SessionIndex) -> Vec<AccountId> {
    // Test assumption
    const FREE_SEATS: u32 = 2;

    let mut non_reserved = vec![];

    let non_reserved_nodes_order_from_runtime = get_non_reserved_members(config);
    let non_reserved_nodes_order_from_runtime_len = non_reserved_nodes_order_from_runtime.len();

    for i in (FREE_SEATS * session)..(FREE_SEATS * (session + 1)) {
        non_reserved.push(
            non_reserved_nodes_order_from_runtime
                [i as usize % non_reserved_nodes_order_from_runtime_len]
                .clone(),
        );
    }

    non_reserved.iter().map(account_from_keypair).collect()
}

fn get_member_accounts(config: &Config) -> (Vec<AccountId>, Vec<AccountId>) {
    (
        get_reserved_members(config)
            .iter()
            .map(account_from_keypair)
            .collect(),
        get_non_reserved_members(config)
            .iter()
            .map(account_from_keypair)
            .collect(),
    )
}

fn validators_bond_extra_stakes(config: &Config, additional_stakes: Vec<Balance>) {
    let node = &config.node;
    let root_connection = config.create_root_connection();

    let validator_seeds = get_validators_seeds(config);
    let controller_seeds: Vec<_> = validator_seeds
        .iter()
        .map(|v| format!("{}//Controller", v))
        .collect();

    let controller_accounts_key_pairs = accounts_seeds_to_keys(&controller_seeds);
    let stash_accounts_key_pairs = accounts_seeds_to_keys(&validator_seeds);

    let controller_accounts: Vec<AccountId> = controller_accounts_key_pairs
        .iter()
        .map(account_from_keypair)
        .collect();

    // funds to cover fees
    balances_batch_transfer(&root_connection.as_signed(), controller_accounts, TOKEN);

    stash_accounts_key_pairs
        .iter()
        .zip(additional_stakes.iter())
        .for_each(|(key, additional_stake)| {
            let validator_id = account_from_keypair(key);

            // Additional TOKEN to cover fees
            balances_transfer(
                &root_connection.as_signed(),
                &validator_id,
                *additional_stake + TOKEN,
                XtStatus::Finalized,
            );
            let stash_connection = SignedConnection::new(node, key.clone());
            let xt = stash_connection
                .as_connection()
                .staking_bond_extra(*additional_stake);
            send_xt(
                &stash_connection,
                xt,
                Some("bond_extra"),
                XtStatus::Finalized,
            );
        });
}

pub fn points_stake_change(config: &Config) -> anyhow::Result<()> {
    const MAX_DIFFERENCE: f64 = 0.05;

    let node = &config.node;
    let accounts = get_validators_keys(config);
    let sender = accounts.first().expect("Using default accounts").to_owned();
    let connection = SignedConnection::new(node, sender);
    let root_connection = config.create_root_connection();

    let (reserved_members, non_reserved_members) = get_member_accounts(config);

    change_validators(
        &root_connection,
        Some(reserved_members.clone()),
        Some(non_reserved_members.clone()),
        Some(4),
        XtStatus::Finalized,
    );

    validators_bond_extra_stakes(
        config,
        [
            8 * MIN_VALIDATOR_BOND,
            6 * MIN_VALIDATOR_BOND,
            4 * MIN_VALIDATOR_BOND,
            2 * MIN_VALIDATOR_BOND,
            0,
        ]
        .to_vec(),
    );

    let session_period = get_session_period(&connection);
    let sessions_per_era = get_sessions_per_era(&connection);

    let era_block = session_period * sessions_per_era * wait_for_next_era(&root_connection)?;
    let era_block_hash = get_block_hash(&connection, era_block);
    let start_era_session = get_session(&connection, Some(era_block_hash));

    let next_era_block = session_period * sessions_per_era * wait_for_next_era(&root_connection)?;
    let next_era_block_hash = get_block_hash(&connection, next_era_block);
    let end_era_session = get_session(&connection, Some(next_era_block_hash));

    info!(
        "Checking rewards for sessions {}..{}.",
        start_era_session, end_era_session
    );

    let era = get_era(&root_connection, Some(era_block_hash));
    for session in start_era_session..end_era_session {
        let non_reserved_for_session = get_non_reserved_members_for_session(config, session);
        let non_reserved_bench = non_reserved_members
            .clone()
            .into_iter()
            .filter(|account_id| !non_reserved_for_session.contains(account_id))
            .collect::<Vec<_>>();
        let members = reserved_members
            .clone()
            .into_iter()
            .chain(non_reserved_for_session)
            .collect::<Vec<_>>();
        let members_bench = non_reserved_bench;

        check_points(
            &connection,
            session,
            era,
            members,
            members_bench,
            MAX_DIFFERENCE,
        )?
    }

    Ok(())
}

pub fn disable_node(config: &Config) -> anyhow::Result<()> {
    const MAX_DIFFERENCE: f64 = 0.05;
    const VALIDATORS_PER_SESSION: u32 = 4;

    let root_connection = config.create_root_connection();

    let sessions_per_era = get_sessions_per_era(&root_connection);

    let (reserved_members, non_reserved_members) = get_member_accounts(config);

    change_validators(
        &root_connection,
        Some(reserved_members.clone()),
        Some(non_reserved_members.clone()),
        Some(VALIDATORS_PER_SESSION),
        XtStatus::Finalized,
    );

    let era = wait_for_next_era(&root_connection)?;
    let start_session = era * sessions_per_era;

    let controller_connection = SignedConnection::new(&config.node, config.node_keys().controller);
    // this should `disable` this node by setting invalid session_keys
    set_invalid_keys_for_validator(&controller_connection)?;
    // this should `re-enable` this node, i.e. by means of the `rotate keys` procedure
    reset_validator_keys(&controller_connection)?;

    let era = wait_for_full_era_completion(&root_connection)?;

    let end_session = era * sessions_per_era;

    info!(
        "Checking rewards for sessions {}..{}.",
        start_session, end_session
    );

    for session in start_session..end_session {
        let non_reserved_for_session = get_non_reserved_members_for_session(config, session);
        let non_reserved_bench = non_reserved_members
            .iter()
            .filter(|account_id| !non_reserved_for_session.contains(*account_id))
            .cloned();

        let members = reserved_members
            .iter()
            .chain(non_reserved_for_session.iter())
            .cloned();
        let members_bench: Vec<_> = non_reserved_bench.collect();

        let era = session / sessions_per_era;
        check_points(
            &controller_connection,
            session,
            era,
            members,
            members_bench,
            MAX_DIFFERENCE,
        )?;
    }

    Ok(())
}
