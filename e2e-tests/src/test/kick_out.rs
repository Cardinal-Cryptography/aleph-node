use aleph_client::{
    change_kickout_config, change_validators, get_committee_kick_out_config,
    get_current_era_validators, get_current_session, get_kick_out_reason_for_validator,
    get_next_era_non_reserved_validators, get_next_era_reserved_validators,
    get_underperformed_validator_session_count, wait_for_at_least_session, wait_for_event,
    wait_for_full_era_completion, AccountId, SignedConnection, XtStatus,
};
use codec::Decode;
use log::info;
use primitives::{
    CommitteeSeats, EraValidators, KickOutReason, SessionCount,
    DEFAULT_CLEAN_SESSION_COUNTER_DELAY, DEFAULT_KICK_OUT_MINIMAL_EXPECTED_PERFORMANCE,
    DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD,
};

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

fn common_setup(
    config: &Config,
) -> anyhow::Result<(EraValidators<AccountId>, EraValidators<AccountId>)> {
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

    Ok((
        start_era_validators,
        EraValidators {
            reserved: reserved_validators,
            non_reserved: non_reserved_validators,
        },
    ))
}

/// Runs a chain, sets up a committee and validators. Sets an incorrect key for one of the
/// validators. Waits for the offending validator to hit the kick-out threshold of sessions without
/// producing blocks. Verifies that the offending validator has in fact been kicked out for the
/// appropriate reason.
pub fn kick_out_automatic(config: &Config) -> anyhow::Result<()> {
    let root_connection = config.create_root_connection();

    let (start_era_validators, next_era_validators) = common_setup(config)?;

    let EraValidators {
        reserved: reserved_validators,
        non_reserved: non_reserved_validators,
    } = next_era_validators;

    let validator_to_disable =
        &non_reserved_validators[VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX as usize];
    info!(target: "aleph-client", "Validator to disable: {}", validator_to_disable);

    let underperformed_validator_session_count =
        get_underperformed_validator_session_count(&root_connection, validator_to_disable);

    assert_eq!(underperformed_validator_session_count, 0);

    let validator_to_disable_kick_out_reason =
        get_kick_out_reason_for_validator(&root_connection, validator_to_disable);

    assert!(validator_to_disable_kick_out_reason.is_none());

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

    let underperformed_validator_session_count =
        get_underperformed_validator_session_count(&root_connection, validator_to_disable);

    assert_eq!(underperformed_validator_session_count, 0);

    let validator_to_disable_kick_out_reason =
        get_kick_out_reason_for_validator(&root_connection, validator_to_disable)
            .expect("Cannot obtain kick-out reason for validator!");
    let expected_kick_out_reason =
        KickOutReason::InsufficientUptime(DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD);

    assert_eq!(
        validator_to_disable_kick_out_reason,
        expected_kick_out_reason
    );

    #[derive(Debug, Decode, Clone)]
    struct KickOutEvent {
        kicked_out_validators: Vec<(AccountId, KickOutReason)>,
    }

    wait_for_event(
        &root_connection,
        ("Elections", "KickOutValidators"),
        |e: KickOutEvent| {
            info!(
                "Received KickOutValidators event: {:?}",
                e.kicked_out_validators
            );
            true
        },
    )?;

    let next_era_reserved_validators = get_next_era_reserved_validators(&root_connection);
    let next_era_non_reserved_validators = get_next_era_non_reserved_validators(&root_connection);

    assert_eq!(next_era_reserved_validators, reserved_validators);
    assert_eq!(
        next_era_non_reserved_validators,
        non_reserved_validators[(VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX + 1) as usize..]
    );

    let current_era_validators = get_current_era_validators(&root_connection);

    assert_eq!(
        current_era_validators.reserved,
        start_era_validators.reserved
    );
    assert_eq!(
        current_era_validators.non_reserved,
        start_era_validators.non_reserved[(VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX + 1) as usize..]
    );

    Ok(())
}

pub fn clearing_session_count(config: &Config) -> anyhow::Result<()> {
    let root_connection = config.create_root_connection();

    let (_, next_era_validators) = common_setup(config)?;

    let EraValidators {
        reserved: reserved_validators,
        non_reserved: non_reserved_validators,
    } = next_era_validators;

    change_kickout_config(&root_connection, None, None, Some(2), XtStatus::InBlock);

    let validator_to_disable =
        &non_reserved_validators[VALIDATOR_TO_DISABLE_NON_RESERVED_INDEX as usize];
    let validator_seed = get_validator_seed(VALIDATOR_TO_DISABLE_OVERALL_INDEX);
    let stash_controller = NodeKeys::from(validator_seed);
    let controller_key_to_disable = stash_controller.controller;

    // This connection has to be set up with the controller key.
    let connection_to_disable =
        SignedConnection::new(NODE_TO_DISABLE_ADDRESS, controller_key_to_disable);

    set_invalid_keys_for_validator(&connection_to_disable)?;

    let current_session = get_current_session(&root_connection);

    wait_for_at_least_session(&root_connection, current_session + 5)?;

    let underperformed_validator_session_count =
        get_underperformed_validator_session_count(&root_connection, validator_to_disable);

    // it only has to be ge than 0 and should be cleared before reaching values larger than 3.
    assert!(underperformed_validator_session_count <= 2);

    let next_era_reserved_validators = get_next_era_reserved_validators(&root_connection);
    let next_era_non_reserved_validators = get_next_era_non_reserved_validators(&root_connection);

    // checks no one was kicked out
    assert_eq!(next_era_reserved_validators, reserved_validators);
    assert_eq!(next_era_non_reserved_validators, non_reserved_validators);

    Ok(())
}
