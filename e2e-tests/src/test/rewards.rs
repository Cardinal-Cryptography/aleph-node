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

    let non_reserved_members: Vec<_> = get_non_reserved_members(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    change_members(
        &root_connection,
        Some(reserved_members.clone()),
        Some(non_reserved_members.clone()),
        Some(4),
        XtStatus::InBlock,
    );

    let reserved_members_performance: Vec<(AccountId, f64)> = reserved_members
        .clone()
        .into_iter()
        .map(|account_id| (account_id, 1.))
        .collect();

    let mut validator_reward_points_previous_session = BTreeMap::new();

    let sessions_per_era: u32 = connection
        .as_connection()
        .get_constant("Staking", "SessionsPerEra")
        .expect("Failed to decode SessionsPerEra extrinsic!");

    let session_period: u32 = connection
        .as_connection()
        .get_constant("Elections", "SessionPeriod")
        .expect("Failed to decode SessionPeriod extrinsic!");

    // If during era 0 we request a controller to be a validator, it becomes one
    // for era 1, and payouts can be collected once era 1 ends,
    // so we want to start the test from era 2.
    wait_for_full_era_completion(&connection)?;
    let session = get_current_session(&connection);

    while session < ERAS * sessions_per_era {
        let session = get_current_session(&connection);
        let era = session / sessions_per_era;

        info!("[+] Era: {} | session: {}", era, session);
        info!("Sessions per era: {}", sessions_per_era);

        let members_per_session: u32 = connection
            .as_connection()
            .get_storage_value("Elections", "MembersPerSession", None)
            .expect("Failed to decode MembersPerSession extrinsic!")
            .unwrap_or_else(|| {
                panic!("Failed to obtain MembersPerSession for session {}", session)
            });

        reserved_members
            .iter()
            .for_each(|account_id| info!("Reserved member: {}", account_id));

        let non_reserved_for_session = get_non_reserved_members_for_session(config, session);
        non_reserved_for_session
            .iter()
            .for_each(|account_id| info!("Non-reserved member in committee: {}", account_id));

        let non_reserved_bench = non_reserved_members
            .iter()
            .filter(|account_id| !non_reserved_for_session.contains(account_id))
            .collect::<Vec<_>>();
        non_reserved_bench
            .iter()
            .for_each(|account_id| info!("Non-reserved member on bench: {}", account_id));

        assert_eq!(
            (reserved_members.len() + non_reserved_for_session.len()) as u32,
            members_per_session
        );

        info!("Members per session: {}", members_per_session);

        let blocks_to_produce_per_session = session_period / members_per_session;
        info!(
            "Blocks to produce per session: {}",
            blocks_to_produce_per_session
        );

        let end_of_session_block = (session + 1) * session_period;
        info!("Waiting for block: {}", end_of_session_block);
        wait_for_finalized_block(&connection, end_of_session_block)?;

        let end_of_session_block_hash = connection
            .as_connection()
            .get_block_hash(Some(end_of_session_block))
            .expect("API call should have succeeded.")
            .unwrap_or_else(|| {
                panic!(
                    "Failed to obtain block hash for block {}.",
                    end_of_session_block
                );
            });
        info!("End-of-session block hash: {}.", end_of_session_block_hash);

        let before_end_of_session_block_hash = connection
            .as_connection()
            .get_block_hash(Some(end_of_session_block - 1))
            .expect("API call should have succeeded.")
            .unwrap_or_else(|| {
                panic!(
                    "Failed to obtain block hash for block {}.",
                    end_of_session_block
                );
            });

        let era_reward_points =
            era_reward_points(&connection, era, Some(end_of_session_block_hash));
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
                    "In session {} validator {} accumulated {}.",
                    session, account_id, reward_points
                )
            });

        let validator_exposures: Vec<(AccountId, u128)> = reserved_members
            .iter()
            .chain(
                non_reserved_members
                    .iter()
            )
            .map( |account_id|
               connection
                   .as_connection()
                   .get_storage_double_map("Staking", "ErasStakers", era, account_id, Some(end_of_session_block_hash))
                   .expect("Failed to decode ErasStakers extrinsic!")
                   .unwrap_or_else(|| panic!("Failed to obtain ErasStakers for session {}.", session))
            ).collect();

        let total_exposure: u128 = validator_exposures
            .iter()
            .map(|(_, exposure)| exposure)
            .sum();

        let reward_scaling_factors: Vec<(AccountId, f64)> = validator_exposures
            .into_iter()
            .map(|(account_id, exposure)| (account_id, (exposure / total_exposure) as f64))
            .collect();

        let non_reserved_for_session_performance: Vec<(AccountId, f64)> = non_reserved_for_session.into_iter().map(|account_id| {
            let block_count: u32 = connection
                .as_connection()
                .get_storage_map(
                    "Elections",
                    "SessionValidatorBlockCount",
                    account_id.clone(),
                    Some(before_end_of_session_block_hash),
                )
                .expect("Failed to decode SessionValidatorBlockCount extrinsic!")
                .unwrap_or_else(|| {
                    panic!(
                        "Failed to obtain SessionValidatorBlockCount for session {}, validator {}, EOS block hash {}.",
                        session, account_id, before_end_of_session_block_hash
                    )
                });
            info!(
                "Block count for validator {} is {:?}, block hash is {}.",
                account_id, block_count, before_end_of_session_block_hash
            );

            let performance = (block_count as f64 / blocks_to_produce_per_session as f64).min(1.);
            info!("Validator {}, performance {}", account_id, performance);

            let lenient_performance =
                match Perquintill::from_percent(performance as u64) > LENIENT_THRESHOLD {
                    true => 1.,
                    false => performance as f64,
                };
            info!(
                "Validator {}, lenient performance {}",
                account_id, lenient_performance
            );
            (account_id, lenient_performance)
        }).collect();

        let adjusted_reward_points: Vec<(AccountId, u32)> = reward_scaling_factors
            .into_iter()
            .zip(
                reserved_members_performance
                    .iter()
                    .chain(non_reserved_for_session_performance.iter()),
            )
            .map(|((account_id, scaling_factor), (_, performance))| {
                let scaled_points = (scaling_factor * performance * MAX_REWARD as f64
                    / sessions_per_era as f64) as u32;
                info!(
                    "Validator {}, adjusted reward points {}",
                    account_id, scaled_points
                );
                (account_id, scaled_points)
            })
            .collect();

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
