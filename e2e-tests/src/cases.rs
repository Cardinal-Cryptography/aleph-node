use std::collections::HashMap;

use crate::test::{
    batch_transactions as test_batch_transactions, change_validators as test_change_validators,
    channeling_fee as test_channeling_fee, fee_calculation as test_fee_calculation,
    finalization as test_finalization, staking_era_payouts as test_staking_era_payouts,
    staking_new_validator as test_staking_new_validator, token_transfer as test_token_transfer,
    treasury_access as test_treasury_access,
};
use crate::config::Config;

/// Wrapper around the type of the test case.
#[derive(Copy, Clone)]
pub enum TestCase {
    BlockNumberResult(fn(&Config) -> anyhow::Result<u32>),
    EmptyResult(fn(&Config) -> anyhow::Result<()>),
}

/// Get a map with test cases that the e2e suite is able to handle.
pub fn possible_test_cases() -> HashMap<&'static str, TestCase> {
    HashMap::from([
       ("finalization", TestCase::BlockNumberResult(test_finalization)),
       ("token_transfer", TestCase::EmptyResult(test_token_transfer)),
       ("channeling_fee", TestCase::EmptyResult(test_channeling_fee)),
       ("treasury_access", TestCase::EmptyResult(test_treasury_access)),
       ("batch_transactions", TestCase::EmptyResult(test_batch_transactions)),
       ("staking_era_payouts", TestCase::EmptyResult(test_staking_era_payouts)),
       ("staking_new_validator", TestCase::EmptyResult(test_staking_new_validator)),
       ("change_validators", TestCase::EmptyResult(test_change_validators)),
       ("fee_calculation", TestCase::EmptyResult(test_fee_calculation)),
    ])
}