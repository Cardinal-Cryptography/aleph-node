use aleph_client::{
    api::elections::events::BanValidators,
    pallets::elections::{ElectionsApi, ElectionsSudoApi},
    primitives::{BanConfig, BanInfo, CommitteeSeats, EraValidators},
    waiting::{AlephWaiting, BlockStatus, WaitingExt},
    AccountId, Connection, RootConnection, TxStatus,
};
use codec::Encode;
use log::info;
use primitives::SessionCount;
use sp_runtime::Perbill;

use crate::{accounts::account_ids_from_keys, validators::get_test_validators, Config};

pub async fn setup_test(
    config: &Config,
) -> anyhow::Result<(RootConnection, Vec<AccountId>, Vec<AccountId>)> {
    let root_connection = config.create_root_connection().await;

    let validator_keys = get_test_validators(config);
    let reserved_validators = account_ids_from_keys(&validator_keys.reserved);
    let non_reserved_validators = account_ids_from_keys(&validator_keys.non_reserved);

    let seats = CommitteeSeats {
        reserved_seats: 2,
        non_reserved_seats: 2,
    };

    root_connection
        .change_validators(
            Some(reserved_validators.clone()),
            Some(non_reserved_validators.clone()),
            Some(seats),
            TxStatus::InBlock,
        )
        .await?;

    root_connection
        .connection
        .wait_for_n_eras(2, BlockStatus::Best)
        .await;

    Ok((
        root_connection,
        reserved_validators,
        non_reserved_validators,
    ))
}

pub fn check_validators(
    expected_reserved: &[AccountId],
    expected_non_reserved: &[AccountId],
    era_validators: EraValidators<AccountId>,
) -> EraValidators<AccountId> {
    assert_eq!(era_validators.reserved, expected_reserved);
    assert_eq!(era_validators.non_reserved, expected_non_reserved);

    era_validators
}

pub async fn check_ban_config(
    connection: &Connection,
    expected_minimal_expected_performance: Perbill,
    expected_session_count_threshold: SessionCount,
    expected_clean_session_counter_delay: SessionCount,
) -> BanConfig {
    let ban_config = connection.get_ban_config(None).await;

    assert_eq!(
        ban_config.minimal_expected_performance.0,
        expected_minimal_expected_performance.deconstruct()
    );
    assert_eq!(
        ban_config.underperformed_session_count_threshold,
        expected_session_count_threshold
    );
    assert_eq!(
        ban_config.clean_session_counter_delay,
        expected_clean_session_counter_delay
    );

    ban_config
}

pub async fn check_underperformed_validator_session_count(
    connection: &Connection,
    validator: &AccountId,
    expected_session_count: SessionCount,
) -> SessionCount {
    let underperformed_validator_session_count = connection
        .get_underperformed_validator_session_count(validator.clone(), None)
        .await
        .unwrap_or_default();

    assert_eq!(
        underperformed_validator_session_count,
        expected_session_count
    );

    underperformed_validator_session_count
}

pub async fn check_underperformed_validator_reason(
    connection: &Connection,
    validator: &AccountId,
    expected_info: Option<&BanInfo>,
) -> Option<BanInfo> {
    let validator_ban_info = connection
        .get_ban_info_for_validator(validator.clone(), None)
        .await;

    match (validator_ban_info.as_ref(), expected_info) {
        (Some(info), Some(expected_info)) => {
            // terrible hack for now :(
            assert_eq!(info.reason.encode(), expected_info.reason.encode());
            assert_eq!(info.start, expected_info.start);

            validator_ban_info
        }
        (None, None) => None,
        _ => panic!(
            "expected infos to be equal: expected {:?}, got {:?}",
            expected_info, validator_ban_info
        ),
    }
}

pub async fn check_ban_event(
    connection: &Connection,
    expected_banned_validators: &[(AccountId, BanInfo)],
) -> anyhow::Result<BanValidators> {
    let event = connection
        .wait_for_event(
            |event: &BanValidators| {
                info!("Received BanValidators event: {:?}", event.0);
                true
            },
            BlockStatus::Best,
        )
        .await;
    assert_eq!(event.0.encode(), expected_banned_validators.encode());

    Ok(event)
}
