#![cfg(test)]

use frame_election_provider_support::{ElectionProvider, Support};

use crate::mock::*;

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
