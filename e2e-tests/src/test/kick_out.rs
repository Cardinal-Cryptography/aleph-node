use aleph_client::{
    change_validators, get_committee_kick_out_config, get_current_era_validators,
    get_current_session, get_kick_out_reason_for_validator, get_next_era_non_reserved_validators,
    get_next_era_reserved_validators, get_next_era_validators,
    get_underperformed_validator_session_count, kick_out_from_committee, wait_for_at_least_session,
    wait_for_event, wait_for_full_era_completion, AccountId, AnyConnection, RootConnection,
    SignedConnection, XtStatus,
};
use codec::Decode;
use log::info;
use primitives::{
    BoundedVec, CommitteeKickOutConfig, CommitteeSeats, EraValidators, KickOutReason, SessionCount,
    DEFAULT_CLEAN_SESSION_COUNTER_DELAY, DEFAULT_KICK_OUT_MINIMAL_EXPECTED_PERFORMANCE,
    DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD,
};
use sp_runtime::Perbill;

use crate::{
    accounts::{account_ids_from_keys, get_validator_seed, NodeKeys},
    rewards::set_invalid_keys_for_validator,
    validators::get_test_validators,
    Config,
};

const VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX: u32 = 0;
const VALIDATOR_TO_DISABLE_OVERALL_INDEX: u32 = 2;
const NODE_TO_DISABLE_ADDRESS: &str = "127.0.0.1:9945";
const SESSIONS_TO_MEET_KICK_OUT_THRESHOLD: SessionCount = 4;

const VALIDATOR_TO_MANUALLY_KICK_OUT_NON_RESERVED_INDEX: u32 = 1;
const VALIDATOR_TO_MANUALLY_KICK_OUT_OVERALL_INDEX: u32 = 3;
const MANUAL_KICK_OUT_REASON: &str = "Manual kick out reason";

/// Runs a chain, sets up a committee and validators. Sets an incorrect key for one of the
/// validators. Waits for the offending validator to hit the kick-out threshold of sessions without
/// producing blocks. Verifies that the offending validator has in fact been kicked out for the
/// appropriate reason.
pub fn kick_out_automatic(config: &Config) -> anyhow::Result<()> {
    let (root_connection, reserved_validators, non_reserved_validators) = setup_test(config)?;

    // Check current era validators.
    let start_era_validators = check_validators(
        &root_connection,
        &reserved_validators,
        &non_reserved_validators,
        get_current_era_validators,
    );

    check_committee_kick_out_config(
        &root_connection,
        DEFAULT_KICK_OUT_MINIMAL_EXPECTED_PERFORMANCE,
        DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD,
        DEFAULT_CLEAN_SESSION_COUNTER_DELAY,
    );

    let validator_to_disable =
        &non_reserved_validators[VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX as usize];
    info!(target: "aleph-client", "Validator to disable: {}", validator_to_disable);

    check_underperformed_validator_session_count(&root_connection, validator_to_disable, 0);
    check_underperformed_validator_reason(&root_connection, validator_to_disable, &None);

    let validator_seed = get_validator_seed(VALIDATOR_TO_DISABLE_OVERALL_INDEX);
    let stash_controller = NodeKeys::from(validator_seed);
    let controller_key_to_disable = stash_controller.controller;

    // This connection has to be set up with the controller key.
    let connection_to_disable =
        SignedConnection::new(NODE_TO_DISABLE_ADDRESS, controller_key_to_disable);

    set_invalid_keys_for_validator(&connection_to_disable)?;

    let current_session = get_current_session(&root_connection);

    wait_for_at_least_session(
        &root_connection,
        current_session + SESSIONS_TO_MEET_KICK_OUT_THRESHOLD,
    )?;

    check_underperformed_validator_session_count(&root_connection, validator_to_disable, 0);
    let expected_kick_out_reason =
        KickOutReason::InsufficientUptime(DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD);
    check_underperformed_validator_reason(
        &root_connection,
        validator_to_disable,
        &Some(expected_kick_out_reason),
    );

    wait_for_kick_out_event(&root_connection)?;

    // Check next era validators.
    check_validators(
        &root_connection,
        &reserved_validators,
        &non_reserved_validators[(VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX + 1) as usize..],
        get_next_era_validators,
    );

    // Check current validators.
    check_validators(
        &root_connection,
        &start_era_validators.reserved,
        &start_era_validators.non_reserved
            [(VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX + 1) as usize..],
        get_current_era_validators,
    );

    Ok(())
}

pub fn kick_out_manual(config: &Config) -> anyhow::Result<()> {
    let root_connection = config.create_root_connection();

    let validator_keys = get_test_validators(config);
    let reserved_validators = account_ids_from_keys(&validator_keys.reserved);
    let non_reserved_validators = account_ids_from_keys(&validator_keys.non_reserved);

    let seats = CommitteeSeats {
        reserved_seats: 2,
        non_reserved_seats: 2,
    };

    change_validators(
        &root_connection,
        Some(reserved_validators.clone()),
        Some(non_reserved_validators.clone()),
        Some(seats),
        XtStatus::InBlock,
    );

    wait_for_full_era_completion(&root_connection)?;

    let start_era_validators = get_current_era_validators(&root_connection);

    assert_eq!(start_era_validators.reserved, reserved_validators);
    assert_eq!(start_era_validators.non_reserved, non_reserved_validators);

    let committee_kick_out_config = get_committee_kick_out_config(&root_connection);

    assert_eq!(
        committee_kick_out_config.minimal_expected_performance,
        DEFAULT_KICK_OUT_MINIMAL_EXPECTED_PERFORMANCE
    );
    assert_eq!(
        committee_kick_out_config.underperformed_session_count_threshold,
        DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD
    );
    assert_eq!(
        committee_kick_out_config.clean_session_counter_delay,
        DEFAULT_CLEAN_SESSION_COUNTER_DELAY
    );

    let validator_to_manually_kick_out =
        &non_reserved_validators[VALIDATOR_TO_MANUALLY_KICK_OUT_NON_RESERVED_INDEX as usize];
    info!(target: "aleph-client", "Validator to disable: {}", validator_to_manually_kick_out);

    let underperformed_validator_session_count = get_underperformed_validator_session_count(
        &root_connection,
        validator_to_manually_kick_out,
    );

    assert_eq!(underperformed_validator_session_count, 0);

    let validator_to_disable_kick_out_reason =
        get_kick_out_reason_for_validator(&root_connection, validator_to_manually_kick_out);

    assert!(validator_to_disable_kick_out_reason.is_none());

    let manual_reason = MANUAL_KICK_OUT_REASON.as_bytes().to_vec();
    let reason = BoundedVec::try_from(manual_reason).expect("Incorrect manual kick out reason!");
    let manual_kick_out_reason = KickOutReason::OtherReason(reason);

    kick_out_from_committee(
        &root_connection,
        validator_to_manually_kick_out,
        manual_kick_out_reason,
        XtStatus::InBlock,
    );

    Ok(())
}

fn setup_test(config: &Config) -> anyhow::Result<(RootConnection, Vec<AccountId>, Vec<AccountId>)> {
    let root_connection = config.create_root_connection();

    let validator_keys = get_test_validators(config);
    let reserved_validators = account_ids_from_keys(&validator_keys.reserved);
    let non_reserved_validators = account_ids_from_keys(&validator_keys.non_reserved);

    let seats = CommitteeSeats {
        reserved_seats: 2,
        non_reserved_seats: 2,
    };

    change_validators(
        &root_connection,
        Some(reserved_validators.clone()),
        Some(non_reserved_validators.clone()),
        Some(seats),
        XtStatus::InBlock,
    );

    wait_for_full_era_completion(&root_connection)?;

    Ok((
        root_connection,
        reserved_validators,
        non_reserved_validators,
    ))
}

fn check_validators<C: AnyConnection>(
    connection: &C,
    expected_reserved: &[AccountId],
    expected_non_reserved: &[AccountId],
    actual_validators_source: fn(&C) -> EraValidators<AccountId>,
) -> EraValidators<AccountId> {
    let era_validators = actual_validators_source(connection);

    assert_eq!(era_validators.reserved, expected_reserved);
    assert_eq!(era_validators.non_reserved, expected_non_reserved);

    era_validators
}

fn check_committee_kick_out_config(
    connection: &RootConnection,
    expected_minimal_expected_performance: Perbill,
    expected_session_count_threshold: SessionCount,
    expected_clean_session_counter_delay: SessionCount,
) -> CommitteeKickOutConfig {
    let committee_kick_out_config = get_committee_kick_out_config(connection);

    assert_eq!(
        committee_kick_out_config.minimal_expected_performance,
        expected_minimal_expected_performance
    );
    assert_eq!(
        committee_kick_out_config.underperformed_session_count_threshold,
        expected_session_count_threshold
    );
    assert_eq!(
        committee_kick_out_config.clean_session_counter_delay,
        expected_clean_session_counter_delay
    );

    committee_kick_out_config
}

fn check_underperformed_validator_session_count<C: AnyConnection>(
    connection: &C,
    validator: &AccountId,
    expected_session_count: SessionCount,
) -> SessionCount {
    let underperformed_validator_session_count =
        get_underperformed_validator_session_count(connection, validator);

    assert_eq!(
        underperformed_validator_session_count,
        expected_session_count
    );

    underperformed_validator_session_count
}

fn check_underperformed_validator_reason<C: AnyConnection>(
    connection: &C,
    validator: &AccountId,
    expected_reason: &Option<KickOutReason>,
) -> Option<KickOutReason> {
    let validator_kick_out_reason = get_kick_out_reason_for_validator(connection, validator);

    assert_eq!(&validator_kick_out_reason, expected_reason);

    validator_kick_out_reason
}

#[derive(Debug, Decode, Clone)]
struct KickOutEvent {
    kicked_out_validators: Vec<(AccountId, KickOutReason)>,
}

fn wait_for_kick_out_event<C: AnyConnection>(connection: &C) -> anyhow::Result<KickOutEvent> {
    let event = wait_for_event(
        connection,
        ("Elections", "KickOutValidators"),
        |e: KickOutEvent| {
            info!(
                "Received KickOutValidators event: {:?}",
                e.kicked_out_validators
            );
            true
        },
    )?;

    Ok(event)
}
