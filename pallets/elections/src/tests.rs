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
