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
        // We check all 4 possibilities for reserved validators:
        // { staking validator, not staking validator } x { any support, no support }.
        //
        // All of them should be elected.
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

#[test]
fn non_reserved_validators_are_included_only_when_validating() {
    new_test_ext(vec![], vec![1, 2, 3, 4]).execute_with(|| {
        // We check all 4 possibilities for non reserved validators:
        // { staking validator, not staking validator } x { any support, no support }.
        //
        // Only those considered as staking should be elected.
        with_electable_targets(vec![1, 2]);
        with_electing_voters(vec![(5, 50, bounded_vec![1]), (6, 60, bounded_vec![3])]);

        let elected = <Elections as ElectionProvider>::elect().expect("`elect()` should succeed");

        assert_eq!(
            elected,
            &[(1, support(50, vec![(5, 50)])), (2, no_support()),]
        );
    });
}
