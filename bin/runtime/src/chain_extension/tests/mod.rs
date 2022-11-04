use std::sync::mpsc::Receiver;

use environments::{InputCorruptedEnvironment, StoreKeyMode, VerifyMode};

use super::*;
use crate::chain_extension::tests::environments::MockedEnvironment;

mod environments;

type RevertibleWeight = i64;

const IDENTIFIER: VerificationKeyIdentifier = [1, 7, 2, 9];

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
    let (env, charging_listener) = InputCorruptedEnvironment::<StoreKeyMode>::new(41, None);
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
fn store_key__too_long_vk() {
    let (env, charging_listener) = InputCorruptedEnvironment::<StoreKeyMode>::new(
        ByteCount::MAX,
        Some(Box::new(|| panic!("Shouldn't read anything at all"))),
    );

    let result = SnarcosChainExtension::snarcos_store_key(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_STORE_KEY_TOO_LONG_KEY))
    ));
    assert_eq!(charged(charging_listener), 0);
}

// #[test]
// #[allow(non_snake_case)]
// fn store_key__positive_scenario() {
//     let content = StoreKeyArgs {
//         key: vec![],
//         identifier: IDENTIFIER,
//     };
//     let (env, charging_listener) = MockedEnvironment::<StoreKeyMode>::new(content.encode());
//     let key_length = env.key_len();
//
//     let result = SnarcosChainExtension::snarcos_store_key(env);
//
//     assert!(matches!(
//         result,
//         Ok(RetVal::Converging(SNARCOS_STORE_KEY_OK))
//     ));
//     assert_eq!(
//         charged(charging_listener),
//         weight_of_store_key(key_length) as RevertibleWeight
//     );
// }

#[test]
#[allow(non_snake_case)]
fn verify__charges_before_reading() {
    let (env, charging_listener) = InputCorruptedEnvironment::<VerifyMode>::new(41, None);

    let result = SnarcosChainExtension::snarcos_verify(env);

    assert!(matches!(result, Err(_)));
    assert_eq!(
        charged(charging_listener),
        weight_of_verify(None) as RevertibleWeight
    );
}
