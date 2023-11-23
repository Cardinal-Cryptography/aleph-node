mod arguments;
mod environment;
mod executor;

use aleph_runtime::Runtime as AlephRuntime;
use pallet_contracts::chain_extension::{ChainExtension, RetVal};

use crate::{
    backend::{
        executor::BackendExecutor,
        tests::{
            arguments::{store_key_args, verify_args},
            environment::{MockedEnvironment, StandardMode, StoreKeyMode, VerifyMode},
            executor::{StoreKeyOkayer, VerifyOkayer},
        },
    },
    status_codes::{STORE_KEY_SUCCESS, VERIFY_SUCCESS},
    BabyLiminalChainExtension,
};

fn simulate_store_key<Exc: BackendExecutor>(expected_ret_val: u32) {
    let env = MockedEnvironment::<StoreKeyMode, StandardMode>::new(store_key_args());
    let result = BabyLiminalChainExtension::<AlephRuntime>::store_key::<Exc, _>(env);
    assert!(matches!(result, Ok(RetVal::Converging(ret_val)) if ret_val == expected_ret_val));
}

fn simulate_verify<Exc: BackendExecutor>(expected_ret_val: u32) {
    let env = MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());
    let result = BabyLiminalChainExtension::<AlephRuntime>::verify::<Exc, _>(env);
    assert!(matches!(result, Ok(RetVal::Converging(ret_val)) if ret_val == expected_ret_val));
}

#[test]
fn extension_is_enabled() {
    assert!(BabyLiminalChainExtension::<AlephRuntime>::enabled())
}

#[test]
#[allow(non_snake_case)]
fn store_key__positive_scenario() {
    simulate_store_key::<StoreKeyOkayer>(STORE_KEY_SUCCESS)
}

#[test]
#[allow(non_snake_case)]
fn verify__positive_scenario() {
    simulate_verify::<VerifyOkayer>(VERIFY_SUCCESS)
}
