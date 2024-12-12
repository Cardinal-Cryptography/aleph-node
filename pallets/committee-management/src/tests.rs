use sp_staking::{EraIndex, SessionIndex};

use crate::mock::{
    active_era, current_era, start_session, BlockNumber, Session, SessionPeriod,
    SessionsPerEra, System, TestBuilderConfig, TestExtBuilder,
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
