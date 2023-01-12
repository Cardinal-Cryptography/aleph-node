use ark_bls12_381::Bls12_381;
use ark_crypto_primitives::SNARK;
use ark_groth16::Groth16;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
use poseidon::hash;

use crate::{preimage::PreimageRelation, CircuitField, GetPublicInput};

#[test]
fn preimage_constraints_correctness() {
    let preimage = CircuitField::from(17u64);
    let image = hash::one_to_one_hash(preimage);

    let circuit = PreimageRelation::with_full_input(preimage, image);

    let cs = ConstraintSystem::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    let is_satisfied = cs.is_satisfied().unwrap();
    if !is_satisfied {
        println!("{:?}", cs.which_is_unsatisfied());
    }

    assert!(is_satisfied);
}

#[test]
fn unsatisfied_preimage_constraints() {
    let true_preimage = CircuitField::from(17u64);
    let fake_image = hash::one_to_one_hash(CircuitField::from(19u64));
    let circuit = PreimageRelation::with_full_input(true_preimage, fake_image);

    let cs = ConstraintSystem::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    let is_satisfied = cs.is_satisfied().unwrap();

    assert!(!is_satisfied);
}

// #[test]
pub fn preimage_proving_and_verifying() {
    let preimage = CircuitField::from(7u64);
    let image = hash::one_to_one_hash(preimage);

    let circuit = PreimageRelation::with_full_input(preimage, image);

    let mut rng = ark_std::test_rng();
    let (pk, vk) = Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();

    let input = circuit.public_input();

    let proof = Groth16::prove(&pk, circuit, &mut rng).unwrap();

    let is_valid = Groth16::verify(&vk, &input, &proof).unwrap();
    assert!(is_valid);
}
