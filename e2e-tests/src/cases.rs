use std::collections::HashMap;

use crate::{
    config::Config,
    test::{
        batch_transactions as test_batch_transactions, change_validators as test_change_validators,
        channeling_fee as test_channeling_fee, fee_calculation as test_fee_calculation,
        finalization as test_finalization, staking_era_payouts as test_staking_era_payouts,
        staking_new_validator as test_staking_new_validator, token_transfer as test_token_transfer,
        treasury_access as test_treasury_access,
    },
};

pub type TestCase = fn(&Config) -> anyhow::Result<()>;

/// Get a map with test cases that the e2e suite is able to handle.
pub fn possible_test_cases() -> HashMap<&'static str, TestCase> {
    HashMap::from([
        ("finalization", test_finalization as TestCase),
        ("token_transfer", test_token_transfer as TestCase),
        ("channeling_fee", test_channeling_fee as TestCase),
        ("treasury_access", test_treasury_access as TestCase),
        ("batch_transactions", test_batch_transactions as TestCase),
        ("staking_era_payouts", test_staking_era_payouts as TestCase),
        (
            "staking_new_validator",
            test_staking_new_validator as TestCase,
        ),
        ("change_validators", test_change_validators as TestCase),
        ("fee_calculation", test_fee_calculation as TestCase),
    ])
}
