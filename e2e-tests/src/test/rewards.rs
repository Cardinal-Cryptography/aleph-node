use crate::{
    accounts::get_validators_keys,
    test::utility::{disable_validator, download_exposure, enable_validator},
    Config,
};
use aleph_client::{
    account_from_keypair, change_validators, get_block_hash, get_current_session,
    get_era_reward_points, get_session_period, get_sessions_per_era, wait_for_at_least_session,
    wait_for_finalized_block, wait_for_full_era_completion, wait_for_next_era, AnyConnection,
    KeyPair, RewardPoint, SignedConnection,
};
use log::info;
use pallet_elections::LENIENT_THRESHOLD;
use primitives::{EraIndex, SessionIndex};
use sp_core::H256;
use sp_runtime::Perquintill;
use std::collections::HashMap;
use substrate_api_client::{AccountId, XtStatus};

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

fn check_rewards(
    validator_reward_points: HashMap<AccountId, f64>,
    retrieved_reward_points: HashMap<AccountId, u32>,
    max_relative_difference: f64,
) -> anyhow::Result<()> {
    let our_sum: f64 = validator_reward_points
        .iter()
        .map(|(_, reward)| reward)
        .sum();
    let retrieved_sum: u32 = retrieved_reward_points
        .iter()
        .map(|(_, reward)| reward)
        .sum();

    for (account, reward) in validator_reward_points {
        let retrieved_reward = *retrieved_reward_points.get(&account).unwrap_or_else(|| {
            panic!(
                "missing account={} in retrieved collection of reward points",
                account
            )
        });

        info!(
            "Retrieved reward for {} is {} - calculated reward is {}.",
            account, retrieved_reward, reward
        );

        let reward_ratio = reward / our_sum;
        let retrieved_ratio = retrieved_reward as f64 / retrieved_sum as f64;

        info!(
            "{} reward_ratio: {}; retrieved_ratio: {}.",
            account, reward_ratio, retrieved_ratio
        );
        assert!((reward_ratio - retrieved_ratio).abs() <= max_relative_difference);
    }

    Ok(())
}

fn get_node_performance(
    connection: &SignedConnection,
    account_id: AccountId,
    before_end_of_session_block_hash: H256,
    blocks_to_produce_per_session: u32,
) -> f64 {
    let block_count: u32 = connection
        .as_connection()
        .get_storage_map(
            "Elections",
            "SessionValidatorBlockCount",
            account_id.clone(),
            Some(before_end_of_session_block_hash),
        )
        .expect("Failed to decode SessionValidatorBlockCount extrinsic!")
        .unwrap_or(0);
    info!(
        "Block count for validator {} is {:?}, block hash is {}.",
        account_id, block_count, before_end_of_session_block_hash
    );
    let performance = block_count as f64 / blocks_to_produce_per_session as f64;
    info!("validator {}, performance {:?}.", account_id, performance);
    let lenient_performance = match Perquintill::from_float(performance) >= LENIENT_THRESHOLD
        && blocks_to_produce_per_session >= block_count
    {
        true => 1.0,
        false => performance,
    };
    info!(
        "Validator {}, lenient performance {:?}.",
        account_id, lenient_performance
    );
    lenient_performance
}

pub fn disable_node(config: &Config) -> anyhow::Result<()> {
    const MAX_DIFFERENCE: f64 = 0.05;
    const VALIDATORS_PER_SESSION: u32 = 4;

    let root_connection = config.create_root_connection();

    let sessions_per_era = get_sessions_per_era(&root_connection);

    let reserved_members: Vec<_> = get_reserved_members(config)
        .iter()
        .map(account_from_keypair)
        .collect();
    let non_reserved_members: Vec<_> = get_non_reserved_members(config)
        .iter()
        .map(account_from_keypair)
        .collect();

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
    disable_validator(&controller_connection)?;
    // force that node to be disabled for around 2 sessions
    let session = get_current_session(&root_connection);
    wait_for_at_least_session(&root_connection, session + 2)?;
    enable_validator(&controller_connection)?;

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

fn check_points(
    connection: &SignedConnection,
    session: SessionIndex,
    era: EraIndex,
    members: impl IntoIterator<Item = AccountId> + Clone,
    members_bench: impl IntoIterator<Item = AccountId> + Clone,
    max_relative_difference: f64,
) -> anyhow::Result<()> {
    let session_period = get_session_period(connection);

    info!("Era: {} | session: {}.", era, session);

    let beggining_of_session_block = session * session_period;
    let end_of_session_block = beggining_of_session_block + session_period;
    info!("Waiting for block: {}.", end_of_session_block);
    wait_for_finalized_block(connection, end_of_session_block)?;

    let beggining_of_session_block_hash = get_block_hash(connection, beggining_of_session_block);
    let end_of_session_block_hash = get_block_hash(connection, end_of_session_block);
    let before_end_of_session_block_hash = get_block_hash(connection, end_of_session_block - 1);
    info!("End-of-session block hash: {}.", end_of_session_block_hash);

    let members_per_session: u32 = connection
        .as_connection()
        .get_storage_value(
            "Elections",
            "CommitteeSize",
            Some(beggining_of_session_block_hash),
        )
        .expect("Failed to decode CommitteeSize extrinsic!")
        .unwrap_or_else(|| panic!("Failed to obtain CommitteeSize for session {}.", session));

    info!("Members per session: {}.", members_per_session);

    let blocks_to_produce_per_session = session_period / members_per_session;
    info!(
        "Blocks to produce per session: {} - session period {}.",
        blocks_to_produce_per_session, session_period
    );

    // get points stored by the Staking pallet
    let validator_reward_points_current_era =
        get_era_reward_points(connection, era, Some(end_of_session_block_hash))
            .unwrap_or_default()
            .individual;

    let validator_reward_points_previous_session =
        get_era_reward_points(connection, era, Some(beggining_of_session_block_hash))
            .unwrap_or_default()
            .individual;

    let validator_reward_points_current_session: HashMap<AccountId, RewardPoint> =
        validator_reward_points_current_era
            .into_iter()
            .map(|(account_id, reward_points)| {
                let reward_points_previous_session = validator_reward_points_previous_session
                    .get(&account_id)
                    .unwrap_or(&0);
                let reward_points_current = reward_points - reward_points_previous_session;

                info!(
                    "In session {} validator {} accumulated {}.",
                    session, account_id, reward_points
                );
                (account_id, reward_points_current)
            })
            .collect();

    let members_uptime = members.into_iter().map(|account_id| {
        (
            account_id.clone(),
            get_node_performance(
                connection,
                account_id,
                before_end_of_session_block_hash,
                blocks_to_produce_per_session,
            ),
        )
    });

    let members_bench_uptime = members_bench
        .into_iter()
        .map(|account_id| (account_id, 1.0));

    let mut reward_points: HashMap<_, _> = members_uptime.chain(members_bench_uptime).collect();
    let members_count = reward_points.len() as f64;
    for (account_id, reward_points) in reward_points.iter_mut() {
        let exposure =
            download_exposure(connection, era, account_id, beggining_of_session_block_hash);
        *reward_points *= exposure as f64 / members_count;
    }

    check_rewards(
        reward_points,
        validator_reward_points_current_session,
        max_relative_difference,
    )
}
