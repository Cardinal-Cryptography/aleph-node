#![cfg(feature = "runtime-std")]
#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]

use aleph_runtime::Runtime as AlephRuntime;
use baby_liminal_extension::BabyLiminalChainExtension;
use pallet_contracts::chain_extension::ChainExtension;

#[test]
fn extension_is_enabled() {
    assert!(BabyLiminalChainExtension::<AlephRuntime>::enabled())
}
