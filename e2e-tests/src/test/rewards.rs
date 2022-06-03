use crate::{
    accounts::{accounts_seeds_to_keys, get_sudo_key, get_validators_keys, get_validators_seeds},
    Config,
};
use aleph_client::{
    change_reserved_members, era_reward_points, get_current_session, wait_for_finalized_block,
    wait_for_full_era_completion, wait_for_session, AnyConnection, Header, KeyPair, RootConnection,
    SignedConnection,
};
use sp_core::Pair;
//use std::collections::HashMap;
use substrate_api_client::{AccountId, XtStatus};

use log::info;

const MINIMAL_TEST_SESSION_START: u32 = 9;
const ELECTION_STARTS: u32 = 6;
const ERAS: u32 = 4;

fn get_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[0..2].to_vec()
}

fn get_non_reserved_members_for_session(config: &Config, session: u32) -> Vec<AccountId> {
    // Test assumption
    const FREE_SEATS: u32 = 2;

    let mut non_reserved = vec![];

    let validators_seeds = get_validators_seeds(config);
    // this order is determined by pallet_staking::ErasStakers::iter_ker_prefix, so by order in
    // map which is not guaranteed, however runtime is deterministic, so we can rely on particular order
    // test needs to be reworked to read order from ErasStakers
    let non_reserved_nodes_order_from_runtime = vec![
        validators_seeds[3].clone(),
        validators_seeds[2].clone(),
        validators_seeds[4].clone(),
    ];
    let non_reserved_nodes_order_from_runtime_len = non_reserved_nodes_order_from_runtime.len();

    for i in (FREE_SEATS * session)..(FREE_SEATS * (session + 1)) {
        non_reserved.push(
            non_reserved_nodes_order_from_runtime
                [i as usize % non_reserved_nodes_order_from_runtime_len]
                .clone(),
        );
    }

    accounts_seeds_to_keys(&non_reserved)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect()
}

fn get_authorities_for_session<C: AnyConnection>(connection: &C, session: u32) -> Vec<AccountId> {
    const SESSION_PERIOD: u32 = 30;
    let first_block = SESSION_PERIOD * session;

    let block = connection
        .as_connection()
        .get_block_hash(Some(first_block))
        .expect("Api call should succeed")
        .expect("Session already started so the first block should be present");

    connection
        .as_connection()
        .get_storage_value("Session", "Validators", Some(block))
        .expect("Api call should succeed")
        .expect("Authorities should always be present")
}

pub fn points_and_payouts(config: &Config) -> anyhow::Result<()> {
    let node = &config.node;
    let accounts = get_validators_keys(config);
    let sender = accounts.first().expect("Using default accounts").to_owned();
    let connection = SignedConnection::new(node, sender);

    let sudo = get_sudo_key(config);

    let root_connection = RootConnection::new(node, sudo);

    let reserved_members: Vec<_> = get_reserved_members(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    change_reserved_members(
        &root_connection,
        reserved_members.clone(),
        XtStatus::InBlock,
    );

    //let mut session_reward_points = HashMap::new();
    let sessions_per_era: u32 = connection
        .as_connection()
        .get_constant("Staking", "SessionsPerEra")
        .expect("Failed to decode SessionsPerEra extrinsic!");

    let session = get_current_session(&connection);
    info!("Session at inception: {}", session);
    // If during era 0 we request a controller to be a validator, it becomes one
    // for era 1, so we want to start the test from era 1.
    wait_for_full_era_completion(&connection)?;

    //while session < ERAS * sessions_per_era {
    loop {
        let session = get_current_session(&connection);
        info!("Current session: {}", session);
        info!("Current session remainder: {}", session % sessions_per_era);
        if session % sessions_per_era != 0 {
            let era = session / sessions_per_era;
            info!("Era: {} | session: {}", era, session);

            let elected = get_authorities_for_session(&connection, session);
            let non_reserved = get_non_reserved_members_for_session(config, session);

            let era_reward_points = era_reward_points(&connection, era);
            let validator_reward_points = era_reward_points.individual;
            validator_reward_points
                .iter()
                .for_each(|(account_id, reward_points)| {
                    info!("Validator {} accumulated {}.", account_id, reward_points)
                });
        }
        wait_for_session(&connection, session + 1)?;
    }

    let block_number = connection
        .as_connection()
        .get_header::<Header>(None)
        .expect("Could not fetch header")
        .expect("Block exists; qed")
        .number;
    wait_for_finalized_block(&connection, block_number)?;

    Ok(())
}
