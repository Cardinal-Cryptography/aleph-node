#![cfg(test)]

use crate::mock::{new_test_ext, run_session};

#[test]
fn test_run_session_for_n_rounds() {
    new_test_ext(&[(1u64, 1u64), (2u64, 2u64)]).execute_with(|| {
        run_session(10);
    });
}
