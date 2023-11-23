mod arguments;
mod environment;

use aleph_runtime::Runtime as AlephRuntime;
use pallet_contracts::chain_extension::{ChainExtension, RetVal};

use crate::{
    backend::{
        executor::BackendExecutor,
        tests::{
            arguments::store_key_args,
            environment::{MockedEnvironment, StandardMode, StoreKeyMode},
        },
    },
    BabyLiminalChainExtension,
};

fn simulate_store_key<Exc: BackendExecutor>(expected_ret_val: u32) {
    let env = MockedEnvironment::<StoreKeyMode, StandardMode>::new(store_key_args());
    let result = BabyLiminalChainExtension::<AlephRuntime>::store_key::<Exc, _>(env);
    assert!(matches!(result, Ok(RetVal::Converging(ret_val)) if ret_val == expected_ret_val));
}

#[test]
fn extension_is_enabled() {
    assert!(BabyLiminalChainExtension::<AlephRuntime>::enabled())
}

#[test]
fn store_key__positive_scenario() {
    // simulate_store_key::<StoreKeyOkayer>(SNARCOS_STORE_KEY_OK)
}
