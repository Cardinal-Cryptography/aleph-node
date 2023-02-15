use ark_bls12_381::Bls12_381;
use ark_crypto_primitives::SNARK;
use ark_ec::bls12::Bls12;
#[cfg(test)]
use ark_ff::BigInteger256;
use ark_ff::Fp256;
use ark_groth16::{Groth16, Proof, VerifyingKey};
#[cfg(test)]
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
use ark_std::vec::Vec;
use liminal_ark_poseidon::hash;

use crate::{preimage::PreimageRelation, relation::state::FullInput, CircuitField, GetPublicInput};

#[test]
fn preimage_constraints_correctness() {
    let preimage = CircuitField::from(17u64);
    let image = hash::one_to_one_hash([preimage]);

    let circuit: PreimageRelation<FullInput> = PreimageRelation::new(Some(preimage), Some(image));

    let cs = ConstraintSystem::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    let is_satisfied = cs.is_satisfied().unwrap();
    assert!(is_satisfied);
}

#[test]
fn unsatisfied_preimage_constraints() {
    let true_preimage = CircuitField::from(17u64);
    let fake_image = hash::one_to_one_hash([CircuitField::from(19u64)]);
    let circuit: PreimageRelation<FullInput> =
        PreimageRelation::new(Some(true_preimage), Some(fake_image));

    let cs = ConstraintSystem::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    let is_satisfied = cs.is_satisfied().unwrap();

    assert!(!is_satisfied);
}

#[test]
pub fn preimage_proving_and_verifying() {
    let (vk, input, proof) = preimage_proving();

    let is_valid = Groth16::verify(&vk, &input, &proof).unwrap();
    assert!(is_valid);
}

#[allow(clippy::type_complexity)]
pub fn preimage_proving() -> (
    VerifyingKey<Bls12<ark_bls12_381::Parameters>>,
    Vec<Fp256<ark_ed_on_bls12_381::FqParameters>>,
    Proof<Bls12<ark_bls12_381::Parameters>>,
) {
    let preimage = CircuitField::from(7u64);
    let image = hash::one_to_one_hash([preimage]);

    let circuit: PreimageRelation<FullInput> = PreimageRelation::new(Some(preimage), Some(image));

    let mut rng = ark_std::test_rng();
    let (pk, vk) = Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();

    let input = circuit.public_input();
    let proof = Groth16::prove(&pk, circuit, &mut rng).unwrap();

    (vk, input, proof)
}

#[test]
pub fn frontend_to_backend_conversion() {
    let frontend_preimage = 7u64;
    let backend_preimage: CircuitField = CircuitField::from(frontend_preimage);
    let expected_backend_hash: CircuitField = hash::one_to_one_hash([backend_preimage]);

    let bint = BigInteger256::new([
        6921429189085971870u64,
        65421081288123788u64,
        1703765854531614015u64,
        5826733087857826612u64,
    ]);

    let actual_backend_hash = CircuitField::new(bint);

    assert_eq!(expected_backend_hash, actual_backend_hash);
}
