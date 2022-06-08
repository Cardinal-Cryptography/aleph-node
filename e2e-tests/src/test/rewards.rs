use crate::{
    accounts::{accounts_seeds_to_keys, get_sudo_key, get_validators_keys, get_validators_seeds},
    Config,
};
use aleph_client::{
    change_members, era_reward_points, get_current_session, wait_for_finalized_block,
    wait_for_full_era_completion, AnyConnection, Header, KeyPair, RewardPoint, RootConnection,
    SignedConnection,
};
use frame_election_provider_support::sp_arithmetic::Perquintill;
use primitives::{LENIENT_THRESHOLD, MAX_REWARD};
use sp_core::Pair;
use std::collections::BTreeMap;
use substrate_api_client::{AccountId, XtStatus};

use log::info;

const ERAS: u32 = 10;

fn get_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[0..2].to_vec()
}

fn get_non_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[2..].to_vec()
}

fn get_non_reserved_members_for_session(config: &Config, session: u32) -> Vec<AccountId> {
    // Test assumption
    const FREE_SEATS: u32 = 2;

    let mut non_reserved = vec![];

    let validators_seeds = get_validators_seeds(config);
    let non_reserved_nodes_order_from_runtime = validators_seeds[2..].to_vec();
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

    let non_reserved_members = get_non_reserved_members(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    change_members(
        &root_connection,
        Some(reserved_members.clone()),
        Some(non_reserved_members),
        Some(4),
        XtStatus::InBlock,
    );

    let mut validator_reward_points_previous_session = BTreeMap::new();

    let sessions_per_era: u32 = connection
        .as_connection()
        .get_constant("Staking", "SessionsPerEra")
        .expect("Failed to decode SessionsPerEra extrinsic!");
    info!("Sessions per era: {}", sessions_per_era);

    let session = get_current_session(&connection);
    info!("Session at inception: {}", session);
    // If during era 0 we request a controller to be a validator, it becomes one
    // for era 1, and payouts can be collected once era 1 ends,
    // so we want to start the test from era 2.
    wait_for_full_era_completion(&connection)?;

    while session < ERAS * sessions_per_era {
        let session = get_current_session(&connection);
        let era = session / sessions_per_era;

        let reserved = get_reserved_members(config);
        let non_reserved_for_session = get_non_reserved_members_for_session(config, session);
        non_reserved_for_session
            .iter()
            .for_each(|account_id| info!("Non-reserved member {}", account_id));

        info!("Era: {} | session: {}", era, session);
        info!("Sessions per era: {}", sessions_per_era);
        let session_period = connection
            .as_connection()
            .get_constant::<u32>("Elections", "SessionPeriod")
            .expect("Failed to decode SessionPeriod extrinsic!");

        let members_per_session = connection
            .as_connection()
            .get_storage_value::<u32>("Elections", "MembersPerSession", None)
            .expect("Failed to decode MembersPerSession extrinsic!")
            .unwrap_or_else(|| {
                panic!("Failed to obtain MembersPerSession for session {}", session)
            });
        assert_eq!(
            (reserved.len() + non_reserved_for_session.len()) as u32,
            members_per_session
        );
        info!("Members per session: {}", members_per_session);

        info!("Waiting for block: {}", (session + 1) * session_period);
        wait_for_finalized_block(&connection, (session + 1) * session_period)?;

        let era_reward_points = era_reward_points(&connection, era);
        let validator_reward_points_current_era = era_reward_points.individual;
        let validator_reward_points_current_session: BTreeMap<AccountId, RewardPoint> =
            validator_reward_points_current_era
                .clone()
                .into_iter()
                .map(|(account_id, reward_points)| {
                    // on era change, there is no previous session within the era,
                    // no subtraction needed
                    let reward_points_current = match session % sessions_per_era {
                        0 => reward_points,
                        _ => {
                            reward_points
                                - validator_reward_points_previous_session
                                    .get(&account_id)
                                    .unwrap_or(&0)
                        }
                    };
                    (account_id, reward_points_current)
                })
                .collect();
        validator_reward_points_current_session
            .iter()
            .for_each(|(account_id, reward_points)| {
                info!(
                    "in session: {} | validator {} accumulated {}.",
                    session, account_id, reward_points
                )
            });
        non_reserved_for_session.iter().for_each(|account_id| {
            let block_count: u32 = connection
                .as_connection()
                .get_storage_map("Elections", "SessionValidatorBlockCount", account_id, None)
                .expect("Failed to decode SessionValidatorBlockCount extrinsic!")
                .unwrap_or_else(|| {
                    panic!(
                        "Failed to obtain SessionValidatorBlockCount for session {}, validator {}.",
                        session, account_id
                    )
                });
            info!("Block count {} for validator {}", block_count, account_id);
            let performance = block_count / session_period;
            let lenient_performance =
                match Perquintill::from_percent(performance as u64) > LENIENT_THRESHOLD {
                    true => 1.,
                    false => performance as f64,
                };
            let exposure: u128 = connection
                .as_connection()
                .get_storage_double_map("Staking", "ErasStakers", era, account_id, None)
                .expect("Failed to decode ErasStakers extrinsic!")
                .unwrap_or_else(|| panic!("Failed to obtain ErasStakers for session {}.", session));
            info!("Exposure {} for validator {}", exposure, account_id);
            let reward_points_per_session =
                lenient_performance * exposure as f64 / sessions_per_era as f64;
            info!(
                "Account {}, adjusted points per session {}",
                account_id, reward_points_per_session
            );
        });
        validator_reward_points_previous_session = validator_reward_points_current_era;
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
