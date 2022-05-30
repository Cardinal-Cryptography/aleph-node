#![cfg(test)]

use frame_election_provider_support::{ElectionProvider, Support};
use frame_support::{
    generate_storage_alias, pallet_prelude::GetStorageVersion, traits::StorageVersion,
};

use crate::{migrations, mock::*, pallet, Config};

generate_storage_alias!(
    Elections, MembersPerSession => Value<u32>
);
generate_storage_alias!(
    Elections, ReservedMembers<T: Config> => Value<Vec<T::AccountId>>
);
generate_storage_alias!(
    Elections, NonReservedMembers<T: Config> => Value<Vec<T::AccountId>>
);
// generate_storage_alias!(
//     Elections, ErasReserved<T: Config> => Value<Vec<T::AccountId>>
// );

#[test]
fn test_elect() {
    new_test_ext(vec![1, 2], vec![]).execute_with(|| {
        let elected = <Elections as ElectionProvider>::elect();
        assert!(elected.is_ok());

        let supp = Support {
            total: 0,
            voters: Vec::new(),
        };

        assert_eq!(elected.unwrap(), &[(1, supp.clone()), (2, supp)]);
    });
}

#[test]
fn migration_from_v0_to_v1_works() {
    new_test_ext(vec![4], vec![1, 2, 3]).execute_with(|| {
        let v0 = <pallet::Pallet<Test> as GetStorageVersion>::on_chain_storage_version();

        assert_eq!(
            v0,
            StorageVersion::default(),
            "Storage version before applying migration should be default",
        );

        let _weight = migrations::v0_to_v1::migrate::<Test, Elections>();

        let v1 = <pallet::Pallet<Test> as GetStorageVersion>::on_chain_storage_version();

        assert_ne!(
            v1,
            StorageVersion::default(),
            "Storage version after applying migration should be incremented"
        );

        assert_eq!(
            MembersPerSession::get(),
            Some(3),
            "Migration should set MembersPerSession to the length of the Members"
        );

        assert_eq!(
            ReservedMembers::<Test>::get(),
            Some(vec![1, 2, 3]),
            "Migration should set ReservedMembers to the content of Members"
        );
        assert_eq!(
            NonReservedMembers::<Test>::get(),
            Some(vec![15]),
            "Migration should set NonReservedMembers to the content of Members"
        );

        // assert_eq!(
        //     ErasReserved::<Test>::get(),
        //     Some(vec![1, 2, 3]),
        //     "Migration should set ErasReserved to the content of Members"
        // );
    })
}
