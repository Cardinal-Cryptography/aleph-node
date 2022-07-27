use std::collections::HashMap;

use aleph_client::{
    change_validators, get_authorities_for_session, get_current_block_number, get_current_session,
    wait_for_finalize,
};
use pallet_elections::CommitteeSeats;
use sp_core::Pair;
use substrate_api_client::{AccountId, XtStatus};

use crate::{
    connection::get_signed_connection,
    validators::{
        get_non_reserved_validators, get_non_reserved_validators_for_session,
        get_reserved_validators,
    },
    Config,
};

const SESSION_PERIOD: u32 = 30;
const TEST_LENGTH: u32 = 5;

pub fn validators_rotate(config: &Config) -> anyhow::Result<()> {
    let connection = get_signed_connection(config);
    let root_connection = config.create_root_connection();

    let reserved_validators: Vec<_> = get_reserved_validators(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    let non_reserved_validators = get_non_reserved_validators(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    change_validators(
        &root_connection,
        Some(reserved_validators.clone()),
        Some(non_reserved_validators),
        Some(CommitteeSeats {
            reserved_seats: 2,
            non_reserved_seats: 2,
        }),
        XtStatus::InBlock,
    );
    wait_for_full_era_completion(&connection)?;

    let current_session = get_current_session(&connection);
    wait_for_session(&connection, current_session + TEST_LENGTH)?;

    let mut non_reserved_count = HashMap::new();

    for session in current_session..current_session + TEST_LENGTH {
        let elected = get_authorities_for_session(&connection, session, SESSION_PERIOD);
        let non_reserved = get_non_reserved_validators_for_session(config, session);

        for nr in non_reserved.clone() {
            *non_reserved_count.entry(nr).or_insert(0) += 1;
        }

        let reserved_included = reserved_validators
            .clone()
            .iter()
            .all(|reserved| elected.contains(reserved));

        let non_reserved_include = non_reserved
            .iter()
            .all(|non_reserved| elected.contains(non_reserved));

        let only_expected_validators = elected
            .iter()
            .all(|elected| reserved_validators.contains(elected) || non_reserved.contains(elected));

        assert!(
            reserved_included,
            "Reserved nodes should always be present, session #{}",
            session
        );
        assert!(
            non_reserved_include,
            "Missing non reserved node, session #{}",
            session
        );
        assert!(
            only_expected_validators,
            "Only expected validators should be present, session #{}",
            session
        );
    }

    let max_elected = non_reserved_count.values().max().unwrap();
    let min_elected = non_reserved_count.values().min().unwrap();
    assert!(max_elected - min_elected <= 1);

    let block_number = get_current_block_number(&connection);
    wait_for_finalized_block(&connection, block_number)?;

    Ok(())
}
