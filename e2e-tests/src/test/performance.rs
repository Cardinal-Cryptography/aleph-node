use std::collections::HashSet;

use aleph_client::{
    pallets::{
        aleph::AlephApi,
        committee_management::CommitteeManagementApi,
        elections::{ElectionsApi, ElectionsSudoApi},
        session::SessionApi,
    },
    primitives::{CommitteeSeats, EraValidators},
    utility::BlocksApi,
    waiting::{BlockStatus, WaitingExt},
    AccountId, RootConnection, TxStatus,
};
use primitives::{
    SessionCount, DEFAULT_FINALITY_BAN_MINIMAL_EXPECTED_PERFORMANCE,
    DEFAULT_FINALITY_BAN_SESSION_COUNT_THRESHOLD,
};

use crate::{accounts::account_ids_from_keys, config::Config, validators::get_test_validators};

const RESERVED_SEATS: u32 = 2;
const NON_RESERVED_SEATS: u32 = 2;

#[tokio::test]
async fn all_validators_have_ideal_performance() -> anyhow::Result<()> {
    let config = crate::config::setup_test();
    let (root_connection, reserved_validators, non_reserved_validators, _) =
        setup_test(config).await?;
    let all_validators = reserved_validators
        .iter()
        .chain(non_reserved_validators.iter());

    check_validators(
        &reserved_validators,
        &non_reserved_validators,
        root_connection.get_current_era_validators(None).await,
    );

    check_ban_config(
        &root_connection,
        DEFAULT_FINALITY_BAN_MINIMAL_EXPECTED_PERFORMANCE,
        DEFAULT_FINALITY_BAN_SESSION_COUNT_THRESHOLD,
    )
    .await;

    for validator in all_validators.clone() {
        check_underperformed_validator_session_count(&root_connection, validator, 0).await
    }

    let session_id = root_connection.get_session(None).await;

    root_connection
        .wait_for_n_sessions(1, BlockStatus::Best)
        .await;

    let block = root_connection.last_block_of_session(session_id).await?;
    let scores = root_connection
        .abft_scores(session_id, block)
        .await
        .unwrap();
    assert!(scores.points.into_iter().all(|point| point <= 1));

    for validator in all_validators {
        check_underperformed_validator_session_count(&root_connection, validator, 0).await
    }
    Ok(())
}

async fn setup_test(
    config: &Config,
) -> anyhow::Result<(
    RootConnection,
    Vec<AccountId>,
    Vec<AccountId>,
    CommitteeSeats,
)> {
    let root_connection = config.create_root_connection().await;

    let validator_keys = get_test_validators(config);
    let reserved_validators = account_ids_from_keys(&validator_keys.reserved);
    let non_reserved_validators = account_ids_from_keys(&validator_keys.non_reserved);

    let seats = CommitteeSeats {
        reserved_seats: RESERVED_SEATS,
        non_reserved_seats: NON_RESERVED_SEATS,
        non_reserved_finality_seats: NON_RESERVED_SEATS,
    };

    root_connection
        .change_validators(
            Some(reserved_validators.clone()),
            Some(non_reserved_validators.clone()),
            Some(seats.clone()),
            TxStatus::InBlock,
        )
        .await?;

    root_connection.wait_for_n_eras(2, BlockStatus::Best).await;

    Ok((
        root_connection,
        reserved_validators,
        non_reserved_validators,
        seats,
    ))
}

fn check_validators(
    expected_reserved: &[AccountId],
    expected_non_reserved: &[AccountId],
    era_validators: EraValidators<AccountId>,
) -> EraValidators<AccountId> {
    assert_eq!(
        HashSet::<_>::from_iter(&era_validators.reserved),
        HashSet::<_>::from_iter(expected_reserved)
    );
    assert_eq!(
        HashSet::<_>::from_iter(&era_validators.non_reserved),
        HashSet::<_>::from_iter(expected_non_reserved)
    );

    era_validators
}

async fn check_ban_config<C: CommitteeManagementApi>(
    connection: &C,
    expected_minimal_expected_performance: u16,
    expected_session_count_threshold: SessionCount,
) {
    let ban_config = connection.get_finality_ban_config(None).await;

    assert_eq!(
        ban_config.minimal_expected_performance,
        expected_minimal_expected_performance
    );
    assert_eq!(
        ban_config.underperformed_session_count_threshold,
        expected_session_count_threshold
    );
}

async fn check_underperformed_validator_session_count<C: CommitteeManagementApi>(
    connection: &C,
    validator: &AccountId,
    expected_session_count: SessionCount,
) {
    let underperformed_validator_session_count = connection
        .get_underperformed_finalizer_session_count(validator.clone(), None)
        .await
        .unwrap_or_default();

    assert_eq!(
        underperformed_validator_session_count,
        expected_session_count
    );
}
