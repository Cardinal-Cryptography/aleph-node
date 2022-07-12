#![cfg(test)]

use frame_election_provider_support::{ElectionProvider, Support};
use frame_support::bounded_vec;

use crate::mock::*;

fn no_support() -> Support<AccountId> {
    Default::default()
}

fn support(total: Balance, voters: Vec<(AccountId, Balance)>) -> Support<AccountId> {
    Support { total, voters }
}

#[test]
fn reserved_validators_are_always_included() {
    new_test_ext(vec![1, 2, 3, 4], vec![]).execute_with(|| {
        with_electable_targets(vec![1, 2]);
        with_electing_voters(vec![(5, 50, bounded_vec![1]), (6, 60, bounded_vec![3])]);

        let elected = <Elections as ElectionProvider>::elect().expect("`elect()` should succeed");

        assert_eq!(
            elected,
            &[
                (1, support(50, vec![(5, 50)])),
                (2, no_support()),
                (3, support(60, vec![(6, 60)])),
                (4, no_support())
            ]
        );
    });
}
