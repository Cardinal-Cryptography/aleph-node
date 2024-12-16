use std::collections::BTreeSet;

use sp_staking::{EraIndex, SessionIndex};

use crate::mock::{
    active_era, start_session, AccountId, BlockNumber, CommitteeManagement, Elections, Session,
    SessionPeriod, SessionsPerEra, System, TestBuilderConfig, TestExtBuilder,
};

fn gen_config() -> TestBuilderConfig {
    TestBuilderConfig {
        reserved_validators: (0..10).collect(),
        non_reserved_validators: (10..100).collect(),
        non_reserved_seats: 50,
        non_reserved_finality_seats: 4,
    }
}

#[test]
fn session_era_work() {
    TestExtBuilder::new(gen_config()).build().execute_with(|| {
        let assert_era_session_block =
            |era_index: EraIndex, session_index: SessionIndex, block_number: BlockNumber| {
                assert_eq!(active_era(), era_index);
                assert_eq!(Session::current_index(), session_index);
                assert_eq!(System::block_number(), block_number);
            };

        assert_era_session_block(0, 0, 1);
        for session_index in 1..=6 {
            start_session(session_index);
            let era_index = session_index / SessionsPerEra::get();
            let block_number = session_index * SessionPeriod::get();
            assert_era_session_block(era_index, session_index, block_number.into());
        }
    })
}

#[test]
fn new_poducers_every_session() {
    TestExtBuilder::new(gen_config()).build().execute_with(|| {
        let mut producers_in_all_sessions = BTreeSet::<BTreeSet<AccountId>>::new();
        for session_index in 2..=6 {
            start_session(session_index);
            let producers = CommitteeManagement::current_session_validators()
                .current
                .producers
                .into_iter()
                .collect();

            assert!(producers_in_all_sessions.insert(producers));
        }
    })
}

#[test]
fn new_finalizers_every_session() {
    TestExtBuilder::new(gen_config()).build().execute_with(|| {
        let mut finalizers_in_all_sessions = BTreeSet::<BTreeSet<AccountId>>::new();
        for session_index in 2..=6 {
            start_session(session_index);
            let finalizers = CommitteeManagement::current_session_validators()
                .current
                .finalizers
                .into_iter()
                .collect();
            assert!(finalizers_in_all_sessions.insert(finalizers));
        }
    })
}

#[test]
fn storage_is_updated_at_the_right_time() {}

#[test]
fn all_reserved_validators_are_chosen() {
    TestExtBuilder::new(gen_config()).build().execute_with(|| {
        let reserved = Elections::current_era_validators().reserved;
        start_session(2);
        let producers: BTreeSet<AccountId> = CommitteeManagement::current_session_validators()
            .current
            .producers
            .into_iter()
            .collect();
        assert!(reserved.iter().all(|rv| producers.contains(rv)));
        let finalizers: BTreeSet<AccountId> = CommitteeManagement::current_session_validators()
            .current
            .finalizers
            .into_iter()
            .collect();
        assert!(reserved.iter().all(|rv| finalizers.contains(rv)));
    })
}

#[test]
fn ban_underperforming_producers() {}

#[test]
fn ban_underperforming_finalizers() {}
