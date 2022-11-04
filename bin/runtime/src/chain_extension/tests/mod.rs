use std::sync::mpsc::Receiver;

use environments::{InputCorruptedEnvironment, StoreKeyMode, VerifyMode};

use super::*;

mod environments;

type RevertibleWeight = i64;

fn vk_bytes() -> &'static [u8] {
    include_bytes!("resources/xor.vk.bytes")
}

fn proof_bytes() -> &'static [u8] {
    include_bytes!("resources/xor.proof.bytes")
}

fn input_bytes() -> &'static [u8] {
    include_bytes!("resources/xor.public_input.bytes")
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
    let (env, charging_listener) = InputCorruptedEnvironment::<StoreKeyMode>::new(41);
    let key_length = env.key_len();
    let result = SnarcosChainExtension::snarcos_store_key(env);
    assert!(matches!(result, Err(_)));
    assert_eq!(
        charged(charging_listener),
        weight_of_store_key(key_length) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__charges_before_reading() {
    let (env, charging_listener) = InputCorruptedEnvironment::<VerifyMode>::new(41);
    let result = SnarcosChainExtension::snarcos_verify(env);
    assert!(matches!(result, Err(_)));
    assert_eq!(
        charged(charging_listener),
        weight_of_verify(None) as RevertibleWeight
    );
}
