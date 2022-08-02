use std::collections::{HashMap, HashSet};

use aleph_client::{
    account_from_keypair, change_validators, get_block_hash, get_current_session, get_era,
    get_era_reward_points, get_era_validators, get_exposure, get_session_period,
    get_session_validators, get_validator_block_count, rotate_keys, set_keys,
    wait_for_at_least_session, wait_for_finalized_block, wait_for_full_era_completion,
    AnyConnection, EraValidators, RewardPoint, SessionKeys, SignedConnection,
};
use log::info;
use pallet_elections::LENIENT_THRESHOLD;
use pallet_staking::Exposure;
use primitives::{CommitteeSeats, EraIndex, SessionIndex};
use sp_core::H256;
use sp_runtime::Perquintill;
use substrate_api_client::{AccountId, XtStatus};

use crate::{accounts::get_validators_keys, Config};

/// Changes session_keys used by a given `controller` to some `zero`/invalid value,
/// making it impossible to create new legal blocks.
pub fn set_invalid_keys_for_validator(
    controller_connection: &SignedConnection,
) -> anyhow::Result<()> {
    const ZERO_SESSION_KEYS: SessionKeys = SessionKeys {
        aura: [0; 32],
        aleph: [0; 32],
    };

    set_keys(controller_connection, ZERO_SESSION_KEYS, XtStatus::InBlock);
    // wait until our node is forced to use new keys, i.e. current session + 2
    let current_session = get_current_session(controller_connection);
    wait_for_at_least_session(controller_connection, current_session + 2)?;

    Ok(())
}

/// Rotates session_keys of a given `controller`, making it able to rejoin the `consensus`.
pub fn reset_validator_keys(controller_connection: &SignedConnection) -> anyhow::Result<()> {
    let validator_keys =
        rotate_keys(controller_connection).expect("Failed to retrieve keys from chain");
    set_keys(controller_connection, validator_keys, XtStatus::InBlock);

    // wait until our node is forced to use new keys, i.e. current session + 2
    let current_session = get_current_session(controller_connection);
    wait_for_at_least_session(controller_connection, current_session + 2)?;

    Ok(())
}

pub fn download_exposure(
    connection: &SignedConnection,
    era: EraIndex,
    account_id: &AccountId,
    beginning_of_session_block_hash: H256,
) -> u128 {
    let exposure: Exposure<AccountId, u128> = get_exposure(
        connection,
        era,
        account_id,
        Some(beginning_of_session_block_hash),
    );
    info!(
        "Validator {} has own exposure of {} and total of {}.",
        account_id, exposure.own, exposure.total
    );
    exposure.others.iter().for_each(|individual_exposure| {
        info!(
            "Validator {} has nominator {} exposure {}.",
            account_id, individual_exposure.who, individual_exposure.value
        )
    });
    exposure.total
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
    account_id: &AccountId,
    before_end_of_session_block_hash: H256,
    blocks_to_produce_per_session: u32,
) -> f64 {
    let block_count = get_validator_block_count(
        connection,
        account_id,
        Some(before_end_of_session_block_hash),
    )
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

pub fn check_points(
    connection: &SignedConnection,
    session: SessionIndex,
    era: EraIndex,
    members: impl IntoIterator<Item = AccountId> + Clone,
    members_bench: impl IntoIterator<Item = AccountId> + Clone,
    members_per_session: u32,
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
                &account_id,
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
pub fn get_bench_members(
    non_reserved_members: &[AccountId],
    non_reserved_members_for_session: &[AccountId],
) -> Vec<AccountId> {
    non_reserved_members
        .into_iter()
        .filter(|account_id| !non_reserved_members_for_session.contains(account_id))
        .cloned()
        .collect::<Vec<_>>()
}

pub fn get_member_accounts<C: AnyConnection>(
    connection: &C,
    session_index: SessionIndex,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let validators = get_era_validators(connection, session_index);
    (validators.reserved, validators.non_reserved)
}

fn get_non_reserved_members_for_session(
    nodes_per_session: u32,
    era_validators: &EraValidators<AccountId>,
    session: SessionIndex,
) -> Vec<AccountId> {
    let non_reserved_len = era_validators.non_reserved.len();
    let free_seats = nodes_per_session - u32::try_from(era_validators.reserved.len()).unwrap();

    let mut non_reserved = Vec::new();

    for i in (free_seats * session)..(free_seats * (session + 1)) {
        non_reserved.push(era_validators.non_reserved[i as usize % non_reserved_len].clone());
    }

    non_reserved
}

pub fn get_era_from_session<C: AnyConnection>(connection: &C, session: SessionIndex) -> EraIndex {
    let session_period = get_session_period(connection);
    let block = session_period * session;
    let block_hash = get_block_hash(connection, block);
    get_era(connection, Some(block_hash))
}

pub fn get_members_for_session<C: AnyConnection>(
    connection: &C,
    members_per_session: u32,
    era_validators: &EraValidators<AccountId>,
    session: SessionIndex,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let non_reserved_members_for_session =
        get_non_reserved_members_for_session(members_per_session, era_validators, session);
    let members_bench = get_bench_members(
        &era_validators.non_reserved,
        &non_reserved_members_for_session,
    );
    let members_active: Vec<_> = era_validators
        .reserved
        .iter()
        .cloned()
        .chain(non_reserved_members_for_session)
        .collect();

    let members_active_set: HashSet<_> = members_active.iter().cloned().collect();
    let network_members: HashSet<_> = get_session_validators(connection, session)
        .into_iter()
        .collect();

    assert_eq!(members_active_set, network_members);

    (members_active, members_bench)
}

pub fn setup_validators(
    config: &Config,
) -> anyhow::Result<(EraValidators<AccountId>, u32, EraIndex)> {
    let root_connection = config.create_root_connection();

    let members: Vec<_> = get_validators_keys(config)
        .iter()
        .map(account_from_keypair)
        .collect();
    // let members = get_session_validators(&root_connection, 0);
    let members_size = members.len();
    let reserved_count = std::cmp::min(members_size / 2, 2);
    let reserved_members = &members[0..reserved_count];
    let non_reserved_members = &members[reserved_count..];

    let reserved_seats = reserved_members.len().try_into().unwrap();
    let non_reserved_seats = (non_reserved_members.len() - 1).try_into().unwrap();
    let members_per_session = reserved_seats + non_reserved_seats;

    change_validators(
        &root_connection,
        Some(reserved_members.into()),
        Some(non_reserved_members.into()),
        Some(CommitteeSeats {
            reserved_seats,
            non_reserved_seats,
        }),
        XtStatus::InBlock,
    );

    let era = wait_for_full_era_completion(&root_connection)?;
    let session = get_current_session(&root_connection);

    let era_validators = EraValidators {
        reserved: reserved_members.to_vec(),
        non_reserved: non_reserved_members.to_vec(),
    };
    let (network_reserved, network_non_reserved) = get_member_accounts(&root_connection, session);
    let reserved: HashSet<_> = era_validators.reserved.iter().cloned().collect();
    let network_reserved: HashSet<_> = network_reserved.into_iter().collect();
    let non_reserved: HashSet<_> = era_validators.non_reserved.iter().cloned().collect();
    let network_non_reserved: HashSet<_> = network_non_reserved.into_iter().collect();

    assert_eq!(reserved, network_reserved);
    assert_eq!(non_reserved, network_non_reserved);

    Ok((era_validators, members_per_session, era))
}
