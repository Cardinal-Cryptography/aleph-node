#![cfg(test)]

use crate::{migrations, mock::*, pallet, Config};
use frame_support::generate_storage_alias;
use frame_support::traits::OneSessionHandler;
use frame_support::traits::{GetStorageVersion, StorageVersion};

generate_storage_alias!(
    Aleph, SessionForValidatorsChange => Value<u32>
);
generate_storage_alias!(
    Aleph, Validators<T: Config> => Value<Vec<T::AccountId>>
);

#[test]
fn migration_from_v0_to_v1_works() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        frame_support::migration::put_storage_value(
            b"Aleph",
            b"SessionForValidatorsChange",
            &[],
            Some(7u32),
        );

        let before = frame_support::migration::get_storage_value::<Option<u32>>(
            b"Aleph",
            b"SessionForValidatorsChange",
            &[],
        );

        assert_eq!(
            before,
            Some(Some(7)),
            "Storage before migration has type Option<u32>"
        );

        frame_support::migration::put_storage_value(
            b"Aleph",
            b"Validators",
            &[],
            Some(vec![AccountId::default()]),
        );

        let v0 = <pallet::Pallet<Test> as GetStorageVersion>::on_chain_storage_version();

        assert_eq!(
            v0,
            StorageVersion::default(),
            "Storage version before applying migration should be default"
        );

        let _weight = migrations::v0_to_v1::migrate::<Test, Aleph>();

        let v1 = <pallet::Pallet<Test> as GetStorageVersion>::on_chain_storage_version();

        assert_ne!(
            v1,
            StorageVersion::default(),
            "Storage version after applying migration should be incremented"
        );

        assert_eq!(
            SessionForValidatorsChange::get(),
            Some(7u32),
            "Migration should preserve ongoing session change with respect to the session number"
        );

        assert_eq!(
            Validators::<Test>::get(),
            Some(vec![AccountId::default()]),
            "Migration should preserve ongoing session change with respect to the validators set"
        );

        let noop_weight = migrations::v0_to_v1::migrate::<Test, Aleph>();
        assert_eq!(
            noop_weight,
            TestDbWeight::get().reads(1),
            "Migration cannot be run twice"
        );
    })
}

#[test]
fn test_update_authorities() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        initialize_session();
        run_session(1);

        Aleph::update_authorities(to_authorities(&[2, 3, 4]).as_slice());

        assert_eq!(Aleph::authorities(), to_authorities(&[2, 3, 4]));
    });
}

#[test]
fn test_initialize_authorities() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        assert_eq!(Aleph::authorities(), to_authorities(&[1, 2]));
    });
}

#[test]
#[should_panic]
fn fails_to_initialize_again_authorities() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        Aleph::initialize_authorities(&to_authorities(&[1, 2, 3]));
    });
}

#[test]
fn test_current_authorities() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        initialize_session();

        run_session(1);

        Aleph::update_authorities(to_authorities(&[2, 3, 4]).as_slice());

        assert_eq!(Aleph::authorities(), to_authorities(&[2, 3, 4]));

        run_session(2);

        Aleph::update_authorities(to_authorities(&[1, 2, 3]).as_slice());

        assert_eq!(Aleph::authorities(), to_authorities(&[1, 2, 3]));
    })
}

#[test]
fn test_session_rotation() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        initialize_session();
        run_session(1);

        let new_validators = new_session_validators(&[3u64, 4u64]);
        let queued_validators = new_session_validators(&[]);
        Aleph::on_new_session(true, new_validators, queued_validators);
        assert_eq!(Aleph::authorities(), to_authorities(&[3, 4]));
    })
}
