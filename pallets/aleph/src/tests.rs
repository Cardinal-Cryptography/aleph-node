#![cfg(test)]

use frame_support::{storage_alias, traits::OneSessionHandler};

use crate::mock::*;

#[storage_alias]
type SessionForValidatorsChange = StorageValue<Aleph, u32>;

#[storage_alias]
type Validators<T> = StorageValue<Aleph, Vec<<T as frame_system::Config>::AccountId>>;

#[cfg(feature = "try-runtime")]
mod migration_tests {
    use std::collections::HashMap;

    use frame_support::{
        storage::migration::{get_storage_value, put_storage_value},
        traits::{GetStorageVersion, StorageVersion},
    };
    use pallets_support::StorageMigration;

    use crate::{migrations, mock::*, pallet};

    const MODULE: &[u8] = b"Aleph";

    #[test]
    fn migration_from_v0_to_v1_works() {
        new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
            put_storage_value(MODULE, b"SessionForValidatorsChange", &[], Some(7u32));
            put_storage_value(MODULE, b"Validators", &[], Some(vec![0u64, 1u64]));

            let _ = migrations::v0_to_v1::Migration::<Test, Aleph>::migrate();
        })
    }

    #[test]
    fn migration_from_v1_to_v2_works() {
        new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
            let map = [
                "SessionForValidatorsChange",
                "Validators",
                "MillisecsPerBlock",
                "SessionPeriod",
            ]
            .iter()
            .zip(0..4)
            .collect::<HashMap<_, _>>();

            map.iter().for_each(|(item, value)| {
                put_storage_value(b"Aleph", item.as_bytes(), &[], value);
            });

            let _weight = migrations::v1_to_v2::Migration::<Test, Aleph>::migrate();

            let v2 = <pallet::Pallet<Test> as GetStorageVersion>::on_chain_storage_version();

            assert_eq!(
                v2,
                StorageVersion::new(2),
                "Storage version after applying migration should be incremented"
            );

            for item in map.keys() {
                assert!(
                    get_storage_value::<i32>(b"Aleph", item.as_bytes(), &[]).is_none(),
                    "Storage item {} should be killed",
                    item
                );
            }
        })
    }
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

#[test]
fn test_emergency_signer() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        initialize_session();

        run_session(1);

        Aleph::set_next_emergency_finalizer(to_authority(&21));

        assert_eq!(Aleph::emergency_finalizer(), None);
        assert_eq!(Aleph::queued_emergency_finalizer(), None);

        run_session(2);

        Aleph::set_next_emergency_finalizer(to_authority(&37));

        assert_eq!(Aleph::emergency_finalizer(), None);
        assert_eq!(Aleph::queued_emergency_finalizer(), Some(to_authority(&21)));

        run_session(3);

        assert_eq!(Aleph::emergency_finalizer(), Some(to_authority(&21)));
        assert_eq!(Aleph::queued_emergency_finalizer(), Some(to_authority(&37)));
    })
}
