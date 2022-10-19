use log::info;

use aleph_client::{change_validators, get_current_era_reserved_validators, get_current_era_non_reserved_validators, wait_for_full_era_completion, XtStatus};
use primitives::CommitteeSeats;

use crate::{
    accounts::account_ids_from_keys,
    validators::get_test_validators,
    Config,
};

pub fn kickout_automatic(config: &Config) -> anyhow::Result<()> {
    let root_connection = config.create_root_connection();

    let validator_count = config.validator_count;
    let era_validators = get_test_validators(config);
    let reserved_validators = account_ids_from_keys(&era_validators.reserved);
    let non_reserved_validators = account_ids_from_keys(&era_validators.non_reserved);

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

    let expected_current_era_validators = EraValidators {
        resereved: reserved_validators,
        non_reserved: non_reserved_validators,
    };
    let current_era_validators = get_current_era_validators(&root_connection);

    assert_eq!(current_era_validators, expected_current_era_validators);

    Ok(())
}