use std::sync::mpsc::Receiver;

use environment::{CorruptedMode, MockedEnvironment, StandardMode, StoreKeyMode, VerifyMode};

use super::*;
use crate::chain_extension::tests::executor::{
    Panicker, StoreKeyErrorer, StoreKeyOkayer, VerifyErrorer, VerifyOkayer,
};

mod environment;
mod executor;

type RevertibleWeight = i64;

const IDENTIFIER: VerificationKeyIdentifier = [1, 7, 2, 9];
const VK: [u8; 2] = [4, 1];
const PROOF: [u8; 20] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3, 2, 3, 8, 4];
const INPUT: [u8; 11] = [0, 5, 7, 7, 2, 1, 5, 6, 6, 4, 9];
const SYSTEM: ProvingSystem = ProvingSystem::Groth16;

fn store_key_args() -> Vec<u8> {
    StoreKeyArgs {
        identifier: IDENTIFIER,
        key: VK.to_vec(),
    }
    .encode()
}

fn verify_args() -> Vec<u8> {
    VerifyArgs {
        identifier: IDENTIFIER,
        proof: PROOF.to_vec(),
        input: INPUT.to_vec(),
        system: SYSTEM,
    }
    .encode()
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
fn store_key__too_much_to_read() {
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
fn store_key__pallet_says_too_long_vk() {
    let (env, charging_listener) =
        MockedEnvironment::<StoreKeyMode, StandardMode>::new(store_key_args());

    let result = SnarcosChainExtension::snarcos_store_key::<
        _,
        StoreKeyErrorer<{ VerificationKeyTooLong }>,
    >(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_STORE_KEY_TOO_LONG_KEY))
    ));
    assert_eq!(
        charged(charging_listener),
        weight_of_store_key(VK.len() as ByteCount) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn store_key__pallet_says_identifier_in_use() {
    let (env, charging_listener) =
        MockedEnvironment::<StoreKeyMode, StandardMode>::new(store_key_args());

    let result = SnarcosChainExtension::snarcos_store_key::<
        _,
        StoreKeyErrorer<{ IdentifierAlreadyInUse }>,
    >(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_STORE_IDENTIFIER_IN_USE))
    ));
    assert_eq!(
        charged(charging_listener),
        weight_of_store_key(VK.len() as ByteCount) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn store_key__positive_scenario() {
    let (env, charging_listener) =
        MockedEnvironment::<StoreKeyMode, StandardMode>::new(store_key_args());

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

#[test]
#[allow(non_snake_case)]
fn verify__pallet_says_proof_deserialization_failed() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result = SnarcosChainExtension::snarcos_verify::<
        _,
        VerifyErrorer<{ DeserializingProofFailed }>,
    >(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_VERIFY_DESERIALIZING_PROOF_FAIL))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__pallet_says_input_deserialization_failed() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result = SnarcosChainExtension::snarcos_verify::<
        _,
        VerifyErrorer<{ DeserializingPublicInputFailed }>,
    >(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_VERIFY_DESERIALIZING_INPUT_FAIL))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__pallet_says_no_such_vk() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result = SnarcosChainExtension::snarcos_verify::<
        _,
        VerifyErrorer<{ UnknownVerificationKeyIdentifier }>,
    >(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_VERIFY_UNKNOWN_IDENTIFIER))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__pallet_says_vk_deserialization_failed() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result = SnarcosChainExtension::snarcos_verify::<
        _,
        VerifyErrorer<{ DeserializingVerificationKeyFailed }>,
    >(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_VERIFY_DESERIALIZING_KEY_FAIL))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__pallet_says_verification_failed() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result =
        SnarcosChainExtension::snarcos_verify::<_, VerifyErrorer<{ VerificationFailed }>>(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_VERIFY_VERIFICATION_FAIL))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__pallet_says_incorrect_proof() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result = SnarcosChainExtension::snarcos_verify::<_, VerifyErrorer<{ IncorrectProof }>>(env);

    assert!(matches!(
        result,
        Ok(RetVal::Converging(SNARCOS_VERIFY_INCORRECT_PROOF))
    ));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}

#[test]
#[allow(non_snake_case)]
fn verify__positive_scenario() {
    let (env, charging_listener) =
        MockedEnvironment::<VerifyMode, StandardMode>::new(verify_args());

    let result = SnarcosChainExtension::snarcos_verify::<_, VerifyOkayer>(env);

    assert!(matches!(result, Ok(RetVal::Converging(SNARCOS_VERIFY_OK))));

    assert_eq!(
        charged(charging_listener),
        weight_of_verify(Some(SYSTEM)) as RevertibleWeight
    );
}
