#![cfg(test)]

use crate::mock::*;
use frame_support::assert_ok;

#[test]
fn test_update_authorities() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        initialize_session();
        run_until_block(1);

        Aleph::update_authorities(to_authorities(&[2, 3, 4]).as_slice());

        assert_eq!(Aleph::authorities(), to_authorities(&[2, 3, 4]));
    });
}

#[test]
fn test_initialize_authorities() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        assert_eq!(Aleph::authorities(), to_authorities(&[1, 2]));
    });
}

#[test]
fn test_validators_should_be_none() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        assert_eq!(Aleph::validators(), None);
    });
}

#[test]
fn test_change_validators_force() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        assert_ok!(Aleph::change_validators(
            Origin::root(),
            vec![AccountId::default()],
            0,
            true
        ));
        assert_eq!(Aleph::validators(), Some(vec![AccountId::default()]));
    });
}

#[test]
fn test_change_validators_non_force() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        assert!(
            Aleph::change_validators(Origin::root(), vec![AccountId::default()], 0, false).is_err()
        );
        assert!(
            Aleph::change_validators(Origin::root(), vec![AccountId::default()], 1, false).is_err()
        );
        assert!(
            Aleph::change_validators(Origin::root(), vec![AccountId::default()], 2, false).is_ok()
        );
    });
}

#[test]
#[should_panic]
fn fails_to_initialize_again_authorities() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        Aleph::initialize_authorities(&to_authorities(&[1, 2, 3]));
    });
}

#[test]
fn test_current_authorities() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        initialize_session();

        run_until_block(1);

        Aleph::update_authorities(to_authorities(&[2, 3, 4]).as_slice());

        assert_eq!(Aleph::authorities(), to_authorities(&[2, 3, 4]));

        run_until_block(2);

        Aleph::update_authorities(to_authorities(&[1, 2, 3]).as_slice());

        assert_eq!(Aleph::authorities(), to_authorities(&[1, 2, 3]),);
    })
}

#[test]
fn test_next_session_authorities() {
    new_test_ext(&[1u64, 2u64]).execute_with(|| {
        initialize_session();

        run_until_block(1);

        assert_eq!(
            Aleph::next_session_authorities().unwrap(),
            to_authorities(&[1, 2])
        );

        run_until_block(2);

        assert_eq!(
            Aleph::next_session_authorities().unwrap(),
            to_authorities(&[1, 2])
        );
    })
}

#[test]
fn test_future_session_validator_ids() {
    new_test_ext(&[0u64, 1u64]).execute_with(|| {
        initialize_session();

        run_until_block(1);

        assert_eq!(Aleph::future_session_validator_ids(1), Ok(vec![0u64, 1u64]));

        run_until_block(2);

        assert_eq!(Aleph::future_session_validator_ids(2), Ok(vec![0u64, 1u64]));

        assert_eq!(Aleph::future_session_validator_ids(3), Ok(vec![0u64, 1u64]));

        assert!(Aleph::future_session_validator_ids(1).is_err());
    })
}

#[test]
fn test_future_session_validator_ids_after_change1() {
    new_test_ext(&[0u64, 1u64]).execute_with(|| {
        initialize_session();
        // We are now in block 1. Even though below we ask for validator rotation in session 0
        // it will happen in session 3 -- the earliest possible.
        Aleph::change_validators(Origin::root(), vec![0u64], 0, true).unwrap();

        assert_eq!(Aleph::future_session_validator_ids(1), Ok(vec![0u64, 1u64]));
        assert_eq!(Aleph::future_session_validator_ids(2), Ok(vec![0u64, 1u64]));
        assert_eq!(Aleph::future_session_validator_ids(3), Ok(vec![0u64]));
        assert_eq!(Aleph::future_session_validator_ids(58), Ok(vec![0u64]));

        run_until_block(2);

        assert!(Aleph::future_session_validator_ids(1).is_err());
        assert_eq!(Aleph::future_session_validator_ids(2), Ok(vec![0u64, 1u64]));
        assert_eq!(Aleph::future_session_validator_ids(3), Ok(vec![0u64]));
        assert_eq!(Aleph::future_session_validator_ids(58), Ok(vec![0u64]));

        run_until_block(3);

        assert!(Aleph::future_session_validator_ids(1).is_err());
        assert!(Aleph::future_session_validator_ids(2).is_err());
        assert_eq!(Aleph::future_session_validator_ids(3), Ok(vec![0u64]));
        assert_eq!(Aleph::future_session_validator_ids(58), Ok(vec![0u64]));
    })
}

#[test]
fn test_future_session_validator_ids_after_change2() {
    new_test_ext(&[0u64, 1u64]).execute_with(|| {
        initialize_session();
        // We are now in block 1
        Aleph::change_validators(Origin::root(), vec![0u64], 15, false).unwrap();

        assert_eq!(
            Aleph::future_session_validator_ids(14),
            Ok(vec![0u64, 1u64])
        );
        assert_eq!(Aleph::future_session_validator_ids(15), Ok(vec![0u64]));
        assert_eq!(Aleph::future_session_validator_ids(16), Ok(vec![0u64]));

        run_until_block(2);

        assert_eq!(
            Aleph::future_session_validator_ids(14),
            Ok(vec![0u64, 1u64])
        );
        assert_eq!(Aleph::future_session_validator_ids(15), Ok(vec![0u64]));
        assert_eq!(Aleph::future_session_validator_ids(16), Ok(vec![0u64]));

        run_until_block(13);

        assert_eq!(
            Aleph::future_session_validator_ids(14),
            Ok(vec![0u64, 1u64])
        );
        assert_eq!(Aleph::future_session_validator_ids(15), Ok(vec![0u64]));
        assert_eq!(Aleph::future_session_validator_ids(16), Ok(vec![0u64]));
    })
}
