use aleph_client::{
    change_validators, get_committee_kick_out_config, get_current_era_validators,
    get_current_session, get_kick_out_reason_for_validator, get_next_era_non_reserved_validators,
    get_next_era_reserved_validators, get_underperformed_validator_session_count,
    wait_for_at_least_session, wait_for_event, wait_for_full_era_completion, AccountId,
    SignedConnection, XtStatus,
};
use codec::Decode;
use log::info;
use primitives::{
    CommitteeSeats, KickOutReason, SessionCount, DEFAULT_CLEAN_SESSION_COUNTER_DELAY,
    DEFAULT_KICK_OUT_MINIMAL_EXPECTED_PERFORMANCE, DEFAULT_KICK_OUT_SESSION_COUNT_THRESHOLD,
};

use crate::{
    accounts::account_ids_from_keys, rewards::set_invalid_keys_for_validator,
    validators::get_test_validators, Config,
};

const NODE_TO_DISABLE_ADDRESS: &str = "127.0.0.1:9943";
const SESSIONS_TO_MEET_KICK_OUT_THRESHOLD: SessionCount = 4;

pub fn kick_out_automatic(config: &Config) -> anyhow::Result<()> {
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

    let validator_to_disable = &non_reserved_validators[0];
    info!("Validator to disable: {}", validator_to_disable);

    let underperformed_validator_session_count =
        get_underperformed_validator_session_count(&root_connection, validator_to_disable);

    assert_eq!(underperformed_validator_session_count, 0);

    let validator_to_disable_kick_out_reason =
        get_kick_out_reason_for_validator(&root_connection, validator_to_disable);

    assert!(validator_to_disable_kick_out_reason.is_none());

    let validator_key_to_disable = validator_keys.non_reserved[0].clone();

    let connection_to_disable =
        SignedConnection::new(NODE_TO_DISABLE_ADDRESS, validator_key_to_disable);
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
            .expect("Cannot obtain kick out reason for validator!");
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
            info!("Received KickOutValidators event: {:?}", e);
            true
        },
    )?;

    let next_era_reserved_validators = get_next_era_reserved_validators(&root_connection);
    let next_era_non_reserved_validators = get_next_era_non_reserved_validators(&root_connection);

    assert_eq!(next_era_reserved_validators, reserved_validators);
    assert_eq!(
        next_era_non_reserved_validators,
        non_reserved_validators[1..]
    );

    let current_era_validators = get_current_era_validators(&root_connection);

    assert_eq!(
        current_era_validators.reserved,
        start_era_validators.reserved
    );
    assert_eq!(
        current_era_validators.non_reserved,
        start_era_validators.non_reserved[1..]
    );

    Ok(())
}
