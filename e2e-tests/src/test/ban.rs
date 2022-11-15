use aleph_client::{
    pallets::{
        elections::{ElectionsApi, ElectionsSudoApi},
        staking::StakingApi,
    },
    primitives::{BanInfo, BanReason},
    waiting::{BlockStatus, WaitingExt},
    SignedConnection, TxStatus,
};
use log::info;
use primitives::{
    SessionCount, DEFAULT_BAN_MINIMAL_EXPECTED_PERFORMANCE, DEFAULT_BAN_SESSION_COUNT_THRESHOLD,
    DEFAULT_CLEAN_SESSION_COUNTER_DELAY,
};

use crate::{
    accounts::{get_validator_seed, NodeKeys},
    ban::{
        check_ban_config, check_ban_event, check_underperformed_validator_reason,
        check_underperformed_validator_session_count, check_validators, setup_test,
    },
    rewards::set_invalid_keys_for_validator,
    Config,
};

const VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX: u32 = 0;
const VALIDATOR_TO_DISABLE_OVERALL_INDEX: u32 = 2;
// Address for //2 (Node2). Depends on the infrastructure setup.
const NODE_TO_DISABLE_ADDRESS: &str = "ws://127.0.0.1:9945";
const SESSIONS_TO_MEET_BAN_THRESHOLD: SessionCount = 4;

async fn disable_validator(validator_address: &str, validator_seed: u32) -> anyhow::Result<()> {
    let validator_seed = get_validator_seed(validator_seed);
    let stash_controller = NodeKeys::from(validator_seed);
    let controller_key_to_disable = stash_controller.controller;

    // This connection has to be set up with the controller key.
    let connection_to_disable =
        SignedConnection::new(validator_address.to_string(), controller_key_to_disable).await;

    set_invalid_keys_for_validator(&connection_to_disable).await
}

/// Runs a chain, sets up a committee and validators. Sets an incorrect key for one of the
/// validators. Waits for the offending validator to hit the ban threshold of sessions without
/// producing blocks. Verifies that the offending validator has in fact been banned out for the
/// appropriate reason.
pub async fn ban_automatic(config: &Config) -> anyhow::Result<()> {
    let (root_connection, reserved_validators, non_reserved_validators) =
        setup_test(config).await?;

    // Check current era validators.
    check_validators(
        &reserved_validators,
        &non_reserved_validators,
        root_connection
            .connection
            .get_current_era_validators(None)
            .await,
    );

    check_ban_config(
        &root_connection.connection,
        DEFAULT_BAN_MINIMAL_EXPECTED_PERFORMANCE,
        DEFAULT_BAN_SESSION_COUNT_THRESHOLD,
        DEFAULT_CLEAN_SESSION_COUNTER_DELAY,
    )
    .await;

    let validator_to_disable =
        &non_reserved_validators[VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX as usize];

    info!(target: "aleph-client", "Validator to disable: {}", validator_to_disable);

    check_underperformed_validator_session_count(
        &root_connection.connection,
        validator_to_disable,
        0,
    )
    .await;
    check_underperformed_validator_reason(&root_connection.connection, validator_to_disable, None)
        .await;

    disable_validator(NODE_TO_DISABLE_ADDRESS, VALIDATOR_TO_DISABLE_OVERALL_INDEX).await?;

    root_connection
        .connection
        .wait_for_n_sessions(SESSIONS_TO_MEET_BAN_THRESHOLD, BlockStatus::Best)
        .await;

    // The session count for underperforming validators is reset to 0 immediately on reaching the
    // threshold.
    check_underperformed_validator_session_count(
        &root_connection.connection,
        validator_to_disable,
        0,
    )
    .await;

    let reason = BanReason::InsufficientUptime(DEFAULT_BAN_SESSION_COUNT_THRESHOLD);
    let start = root_connection.connection.get_current_era(None).await + 1;
    let expected_ban_info = BanInfo { reason, start };

    check_underperformed_validator_reason(
        &root_connection.connection,
        validator_to_disable,
        Some(&expected_ban_info),
    )
    .await;

    let expected_non_reserved =
        &non_reserved_validators[(VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX + 1) as usize..];

    let expected_banned_validators = vec![(validator_to_disable.clone(), expected_ban_info)];
    check_ban_event(&root_connection.connection, &expected_banned_validators).await?;

    // Check current validators.
    check_validators(
        &reserved_validators,
        expected_non_reserved,
        root_connection
            .connection
            .get_current_era_validators(None)
            .await,
    );

    Ok(())
}

/// Setup validators and non_validators. Set ban config clean_session_counter_delay to 2, while
/// underperformed_session_count_threshold to 3.
/// Disable one non_reserved validator. Check if the disabled validator is still in the committee
/// and his underperformed session count is less or equal to 2.
pub async fn clearing_session_count(config: &Config) -> anyhow::Result<()> {
    let (root_connection, reserved_validators, non_reserved_validators) =
        setup_test(config).await?;

    info!(target: "aleph-client", "changing ban config");

    root_connection
        .change_ban_config(None, Some(3), Some(2), None, TxStatus::InBlock)
        .await?;

    let validator_to_disable =
        &non_reserved_validators[VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX as usize];
    info!(target: "aleph-client", "Disabling validator {}", validator_to_disable);
    disable_validator(NODE_TO_DISABLE_ADDRESS, VALIDATOR_TO_DISABLE_OVERALL_INDEX).await?;

    root_connection
        .connection
        .wait_for_n_sessions(5, BlockStatus::Best)
        .await;

    let underperformed_validator_session_count = root_connection
        .connection
        .get_underperformed_validator_session_count(validator_to_disable.clone(), None)
        .await
        .unwrap_or_default();

    // it only has to be ge than 0 and should be cleared before reaching values larger than 3.
    assert!(underperformed_validator_session_count <= 2);

    let next_era_reserved_validators = root_connection
        .connection
        .get_next_era_reserved_validators(None)
        .await;
    let next_era_non_reserved_validators = root_connection
        .connection
        .get_next_era_non_reserved_validators(None)
        .await;

    // checks no one was kicked out
    assert_eq!(next_era_reserved_validators, reserved_validators);
    assert_eq!(next_era_non_reserved_validators, non_reserved_validators);

    Ok(())
}
