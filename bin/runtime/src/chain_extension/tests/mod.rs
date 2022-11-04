use std::sync::mpsc::Receiver;

use environment::{CorruptedMode, MockedEnvironment, StandardMode, StoreKeyMode, VerifyMode};
use pallet_snarcos::ProvingSystem::Groth16;

use super::*;
use crate::chain_extension::tests::executor::{Panicker, StoreKeyOkayer};

mod environment;
mod executor;

type RevertibleWeight = i64;

const IDENTIFIER: VerificationKeyIdentifier = [1, 7, 2, 9];
const VK: [u8; 2] = [4, 1];
const PROOF: [u8; 20] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3, 2, 3, 8, 4];
const INPUT: [u8; 11] = [0, 5, 7, 7, 2, 1, 5, 6, 6, 4, 9];

fn store_key_args() -> StoreKeyArgs {
    StoreKeyArgs {
        identifier: IDENTIFIER,
        key: VK.to_vec(),
    }
}

fn verify_args() -> VerifyArgs {
    VerifyArgs {
        identifier: IDENTIFIER,
        proof: PROOF.to_vec(),
        input: INPUT.to_vec(),
        system: Groth16,
    }
}

fn charged(charging_listener: Receiver<RevertibleWeight>) -> RevertibleWeight {
    charging_listener.into_iter().sum()
}

#[test]
fn extension_is_enabled() {
    assert!(SnarcosChainExtension::enabled())
}

#[test]
#[allow(non_snake_case)]
fn store_key__charges_before_reading() {
    let (env, charging_listener) = MockedEnvironment::<StoreKeyMode, CorruptedMode>::new(41, None);
    let key_length = env.approx_key_len();

    let result = SnarcosChainExtension::snarcos_store_key::<_, Panicker>(env);

    assert!(matches!(result, Err(_)));
    assert_eq!(
        charged(charging_listener),
        weight_of_store_key(key_length) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn store_key__too_long_vk() {
    let (env, charging_listener) = MockedEnvironment::<StoreKeyMode, CorruptedMode>::new(
        ByteCount::MAX,
        Some(Box::new(|| panic!("Shouldn't read anything at all"))),
    );

    let result = SnarcosChainExtension::snarcos_store_key::<_, Panicker>(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_STORE_KEY_TOO_LONG_KEY))
    ));
    assert_eq!(charged(charging_listener), 0);
}

#[test]
#[allow(non_snake_case)]
fn store_key__positive_scenario() {
    let (env, charging_listener) =
        MockedEnvironment::<StoreKeyMode, StandardMode>::new(store_key_args().encode());

    let result = SnarcosChainExtension::snarcos_store_key::<_, StoreKeyOkayer>(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_STORE_KEY_OK))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_store_key(VK.len() as ByteCount) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__charges_before_reading() {
    let (env, charging_listener) = MockedEnvironment::<VerifyMode, CorruptedMode>::new(41, None);

    let result = SnarcosChainExtension::snarcos_verify::<_, Panicker>(env);

    assert!(matches!(result, Err(_)));
    assert_eq!(
        charged(charging_listener),
        weight_of_verify(None) as RevertibleWeight
    );
}
