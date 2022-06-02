use crate::{
    accounts::{accounts_from_seeds, get_sudo},
    Config,
};
use aleph_client::{
    change_reserved_members, era_reward_points, get_current_session, wait_for_era_completion,
    wait_for_finalized_block, wait_for_session, AnyConnection, Header, KeyPair, RootConnection,
    SignedConnection,
};
use sp_core::Pair;
//use std::collections::HashMap;
use substrate_api_client::{AccountId, XtStatus};

use log::info;

const MINIMAL_TEST_SESSION_START: u32 = 9;
const ELECTION_STARTS: u32 = 6;
const ERAS: u32 = 4;

fn get_reserved_members() -> Vec<KeyPair> {
    accounts_from_seeds(&Some(vec!["//Damian".to_string(), "//Tomasz".to_string()]))
}

fn get_non_reserved_members_for_session(session: u32) -> Vec<AccountId> {
    // Test assumption
    const FREE_SEATS: u32 = 2;

    let mut non_reserved = vec![];

    let x = vec![
        "//Julia".to_string(),
        "//Zbyszko".to_string(),
        "//Hansu".to_string(),
    ];
    let x_len = x.len();

    for i in (FREE_SEATS * session)..(FREE_SEATS * (session + 1)) {
        non_reserved.push(x[i as usize % x_len].clone());
    }

    accounts_from_seeds(&Some(non_reserved))
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

pub fn points_and_payouts(cfg: &Config) -> anyhow::Result<()> {
    let node = &cfg.node;
    let accounts = accounts_from_seeds(&None);
    let sender = accounts.first().expect("Using default accounts").to_owned();
    let connection = SignedConnection::new(node, sender);

    let sudo = get_sudo(cfg);

    let root_connection = RootConnection::new(node, sudo);

    let reserved_members: Vec<_> = get_reserved_members()
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
    wait_for_era_completion(&connection, 2)?;
    let session = get_current_session(&connection);
    info!("Session at test start: {}", session);

    while session < ERAS * sessions_per_era {
        let era = session / sessions_per_era;
        info!("In era: {}", era);
        info!("Processing session: {}", session);
        let elected = get_authorities_for_session(&connection, session);
        let non_reserved = get_non_reserved_members_for_session(session);

        let era_reward_points = era_reward_points(&connection, era)
            .unwrap_or_else(|| panic!("Failed to obtain EraRewardPoints for era {}, session {}", era, session));
        let validator_reward_points = era_reward_points.individual;
        validator_reward_points.iter().for_each(|(account_id, reward_points)| info!("Validator {} accumulated {}.", account_id, reward_points));
        let session = wait_for_session(&connection, session + 1)?;
        info!("Next session: {}", session);
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
