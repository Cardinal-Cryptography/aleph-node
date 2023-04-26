use frame_support::{assert_err, assert_ok, sp_runtime, traits::ReservableCurrency, BoundedVec};
use frame_system::{pallet_prelude::OriginFor, Config};
use sp_runtime::traits::Get;

use super::setup::*;
use crate::{
    Error, KeyPairDeposits, KeyPairIdentifier, KeyPairOwners, ProvingVerificationKeyPairs,
    VerificationError,
};

type BabyLiminal = crate::Pallet<TestRuntime>;

const IDENTIFIER: KeyPairIdentifier = [0; 8];

fn pk() -> Vec<u8> {
    include_bytes!("../resources/groth16/xor.pk.bytes").to_vec()
}

fn vk() -> Vec<u8> {
    include_bytes!("../resources/groth16/xor.vk.bytes").to_vec()
}

fn proof() -> Vec<u8> {
    include_bytes!("../resources/groth16/xor.proof.bytes").to_vec()
}

fn input() -> Vec<u8> {
    include_bytes!("../resources/groth16/xor.public_input.bytes").to_vec()
}

fn owner() -> OriginFor<TestRuntime> {
    <TestRuntime as Config>::RuntimeOrigin::signed(1)
}

fn not_owner() -> OriginFor<TestRuntime> {
    <TestRuntime as Config>::RuntimeOrigin::signed(2)
}

fn reserved_balance(account_id: u128) -> u64 {
    <TestRuntime as crate::Config>::Currency::reserved_balance(&account_id)
}

fn free_balance(account_id: u128) -> u64 {
    <TestRuntime as crate::Config>::Currency::free_balance(&account_id)
}

fn put_key() -> u64 {
    let owner = 1;

    let pk = BoundedVec::try_from(pk()).unwrap();
    let vk = BoundedVec::try_from(vk()).unwrap();

    let per_byte_fee: u64 = <TestRuntime as crate::Config>::KeyPairDepositPerByte::get();
    let deposit = (pk.len() + vk.len()) as u64 * per_byte_fee;

    let key_pair = (pk, vk);

    ProvingVerificationKeyPairs::<TestRuntime>::insert(IDENTIFIER, key_pair);
    KeyPairOwners::<TestRuntime>::insert(IDENTIFIER, owner);
    KeyPairDeposits::<TestRuntime>::insert((owner, IDENTIFIER), deposit);
    <TestRuntime as crate::Config>::Currency::reserve(&owner, deposit)
        .expect("Could not reserve a deposit");
    deposit
}

#[test]
fn stores_key_pair_with_fresh_identifier() {
    new_test_ext().execute_with(|| {
        let proving_key = pk();
        let verification_key = vk();
        assert_ok!(BabyLiminal::store_key_pair(
            owner(),
            IDENTIFIER,
            proving_key.clone(),
            verification_key.clone(),
        ));

        let stored_key_pair = ProvingVerificationKeyPairs::<TestRuntime>::get(IDENTIFIER);
        let (pk, vk) = stored_key_pair.unwrap();
        assert_eq!(pk.to_vec(), proving_key);
        assert_eq!(vk.to_vec(), verification_key);
    });
}

#[test]
fn does_not_overwrite_registered_key() {
    new_test_ext().execute_with(|| {
        put_key();

        assert_err!(
            BabyLiminal::store_key_pair(owner(), IDENTIFIER, pk(), vk()),
            Error::<TestRuntime>::IdentifierAlreadyInUse
        );
    });
}

#[test]
fn not_owner_cannot_delete_key() {
    new_test_ext().execute_with(|| {
        put_key();
        assert_err!(
            BabyLiminal::delete_key_pair(not_owner(), IDENTIFIER),
            Error::<TestRuntime>::NotOwner
        );
    });
}

#[test]
fn owner_can_delete_key() {
    new_test_ext().execute_with(|| {
        put_key();
        assert_ok!(BabyLiminal::delete_key_pair(owner(), IDENTIFIER));
    });
}

#[test]
fn not_owner_cannot_overwrite_key() {
    new_test_ext().execute_with(|| {
        put_key();
        assert_err!(
            BabyLiminal::overwrite_key_pair(not_owner(), IDENTIFIER, pk(), vk()),
            Error::<TestRuntime>::NotOwner
        );
    });
}

#[test]
fn owner_can_overwrite_key() {
    new_test_ext().execute_with(|| {
        put_key();
        assert_ok!(BabyLiminal::overwrite_key_pair(
            owner(),
            IDENTIFIER,
            pk(),
            vk()
        ));
    });
}

#[test]
fn key_deposits() {
    new_test_ext().execute_with(|| {
        let per_byte_fee: u64 = <TestRuntime as crate::Config>::KeyPairDepositPerByte::get();

        let reserved_balance_begin = reserved_balance(1);
        let deposit = put_key();
        let reserved_balance_after = reserved_balance(1);

        assert_eq!(reserved_balance_begin + deposit, reserved_balance_after);

        let long_proving_key_len = 2 * pk().len();
        let long_proving_key = vec![0u8; long_proving_key_len];

        let long_verification_key_len = 2 * vk().len();
        let long_verification_key = vec![0u8; long_verification_key_len];

        let free_balance_before = free_balance(1);
        assert_ok!(BabyLiminal::overwrite_key_pair(
            owner(),
            IDENTIFIER,
            long_proving_key,
            long_verification_key
        ));

        let balance_change =
            ((long_proving_key_len + long_verification_key_len) as u64 * per_byte_fee) - deposit;

        assert_eq!(free_balance_before - free_balance(1), balance_change,);

        let short_proving_key_len = vk().len() / 2;
        let short_proving_key = vec![0u8; short_proving_key_len];

        let short_verification_key_len = vk().len() / 2;
        let short_verification_key = vec![0u8; short_verification_key_len];

        let reserved_balance_before = reserved_balance(1);
        assert_ok!(BabyLiminal::overwrite_key_pair(
            owner(),
            IDENTIFIER,
            short_proving_key,
            short_verification_key
        ));
        let reserved_balance_after = reserved_balance(1);

        let long_key_pair_len = long_proving_key_len + long_verification_key_len;
        let short_key_pair_len = short_proving_key_len + short_verification_key_len;
        let reserved_balance_change =
            (long_key_pair_len - short_key_pair_len) as u64 * per_byte_fee;
        assert_eq!(
            reserved_balance_before - reserved_balance_after,
            reserved_balance_change,
        );

        assert_ok!(BabyLiminal::delete_key_pair(owner(), IDENTIFIER));
        assert_eq!(reserved_balance_begin, reserved_balance(1));
    });
}

#[test]
fn does_not_store_too_long_proving_key() {
    new_test_ext().execute_with(|| {
        let proving_key_limit: u32 = <TestRuntime as crate::Config>::MaximumProvingKeyLength::get();

        assert_err!(
            BabyLiminal::store_key_pair(
                owner(),
                IDENTIFIER,
                vec![0; (proving_key_limit + 1) as usize],
                vec![0; 1]
            ),
            Error::<TestRuntime>::ProvingKeyTooLong
        );
    });
}

#[test]
fn does_not_store_too_long_verification_key() {
    new_test_ext().execute_with(|| {
        let verification_key_limit: u32 =
            <TestRuntime as crate::Config>::MaximumVerificationKeyLength::get();

        assert_err!(
            BabyLiminal::store_key_pair(
                owner(),
                IDENTIFIER,
                vec![0; 1],
                vec![0; (verification_key_limit + 1) as usize]
            ),
            Error::<TestRuntime>::VerificationKeyTooLong
        );
    });
}

#[test]
fn verifies_proof() {
    new_test_ext().execute_with(|| {
        put_key();

        assert_ok!(BabyLiminal::verify(owner(), IDENTIFIER, proof(), input(),));
    });
}

#[test]
fn verify_shouts_when_data_is_too_long() {
    new_test_ext().execute_with(|| {
        let limit: u32 = <TestRuntime as crate::Config>::MaximumDataLength::get();

        let result =
            BabyLiminal::verify(owner(), IDENTIFIER, vec![0; (limit + 1) as usize], input());
        assert_err!(
            result.map_err(|e| e.error),
            Error::<TestRuntime>::DataTooLong
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_some());

        let result =
            BabyLiminal::verify(owner(), IDENTIFIER, proof(), vec![0; (limit + 1) as usize]);
        assert_err!(
            result.map_err(|e| e.error),
            Error::<TestRuntime>::DataTooLong
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_some());
    });
}

#[test]
fn verify_shouts_when_no_key_was_registered() {
    new_test_ext().execute_with(|| {
        let result = BabyLiminal::verify(owner(), IDENTIFIER, proof(), input());

        assert_err!(
            result.map_err(|e| e.error),
            Error::<TestRuntime>::UnknownKeyPairIdentifier
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_some());
    });
}

#[test]
fn verify_shouts_when_verification_key_is_not_deserializable() {
    new_test_ext().execute_with(|| {
        let pk = BoundedVec::try_from(pk()).unwrap();

        ProvingVerificationKeyPairs::<TestRuntime>::insert(
            IDENTIFIER,
            (pk, BoundedVec::try_from(vec![0, 1, 2]).unwrap()),
        );

        let result = BabyLiminal::verify(owner(), IDENTIFIER, proof(), input());

        assert_err!(
            result.map_err(|e| e.error),
            Error::<TestRuntime>::DeserializingVerificationKeyFailed
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_some());
    });
}

#[test]
fn verify_shouts_when_proof_is_not_deserializable() {
    new_test_ext().execute_with(|| {
        put_key();

        let result = BabyLiminal::verify(owner(), IDENTIFIER, input(), input());

        assert_err!(
            result.map_err(|e| e.error),
            Error::<TestRuntime>::DeserializingProofFailed
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_some());
    });
}

#[test]
fn verify_shouts_when_input_is_not_deserializable() {
    new_test_ext().execute_with(|| {
        put_key();

        let result = BabyLiminal::verify(owner(), IDENTIFIER, proof(), proof());

        assert_err!(
            result.map_err(|e| e.error),
            Error::<TestRuntime>::DeserializingPublicInputFailed
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_some());
    });
}

#[test]
fn verify_shouts_when_verification_fails() {
    new_test_ext().execute_with(|| {
        put_key();
        let other_input = include_bytes!("../resources/groth16/linear_equation.public_input.bytes");

        let result = BabyLiminal::verify(owner(), IDENTIFIER, proof(), other_input.to_vec());

        assert_err!(
            result,
            Error::<TestRuntime>::VerificationFailed(VerificationError::MalformedVerifyingKey)
        );
        assert!(result.unwrap_err().post_info.actual_weight.is_none());
    });
}

#[test]
fn verify_shouts_when_proof_is_incorrect() {
    new_test_ext().execute_with(|| {
        put_key();
        let other_proof = include_bytes!("../resources/groth16/linear_equation.proof.bytes");

        let result = BabyLiminal::verify(owner(), IDENTIFIER, other_proof.to_vec(), input());

        assert_err!(result, Error::<TestRuntime>::IncorrectProof);
        assert!(result.unwrap_err().post_info.actual_weight.is_none());
    });
}
