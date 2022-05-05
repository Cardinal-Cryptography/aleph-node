use std::collections::HashMap;

use crate::test::{
    batch_transactions as test_batch_transactions, change_validators as test_change_validators,
    channeling_fee as test_channeling_fee, fee_calculation as test_fee_calculation,
    finalization as test_finalization, staking_era_payouts as test_staking_era_payouts,
    staking_new_validator as test_staking_new_validator, token_transfer as test_token_transfer,
    treasury_access as test_treasury_access,
};
use crate::config::Config;

/// Get a map with test cases that the e2e suite is able to handle.
pub fn possible_test_cases() -> HashMap<&'static str, fn(&Config) -> anyhow::Result<()>> {
    HashMap::from([
       ("finalization", test_finalization),
       ("token_transfer", test_token_transfer),
       ("channeling_fee", test_channeling_fee),
       ("treasury_access", test_treasury_access),
       ("batch_transactions", test_batch_transactions),
       ("staking_era_payouts", test_staking_era_payouts),
       ("staking_new_validator", test_staking_new_validator),
       ("change_validators", test_change_validators),
       ("fee_calculation", test_fee_calculation),
    ])
}