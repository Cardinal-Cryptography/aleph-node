use std::ops::MulAssign;

use ark_bls12_377::{Bls12_377, Fr};
use ark_ed_on_bls12_377::Fr as FrEd;
use ark_groth16::Groth16;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef};
use ark_snark::SNARK;
use ark_std::UniformRand;
use criterion::{criterion_group, criterion_main, Criterion};
use liminal_ark_relations::poe::{generator, PoE};

fn poe(c: &mut Criterion) {
    let mut rng = ark_std::test_rng();
    let exp = FrEd::rand(&mut rng);
    let mut generator = generator();
    generator.mul_assign(exp);
    let point = generator;

    let circuit = PoE::new(point.x, point.y, exp);
    let cs: ConstraintSystemRef<Fr> = ConstraintSystem::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    println!("number of constraints    {:?}", cs.num_constraints());
    println!("number of witness vars   {:?}", cs.num_witness_variables());
    println!("number of instance vars  {:?}", cs.num_instance_variables());

    let circuit = PoE::new(point.x, point.y, exp);
    let (pk, vk) = Groth16::<Bls12_377>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
    let input = circuit.public_input();
    let proof = Groth16::prove(&pk, circuit, &mut rng).unwrap();
    let valid_proof = Groth16::verify(&vk, &input, &proof).unwrap();
    assert!(valid_proof);

    c.bench_function("poe", |f| {
        f.iter(|| {
            let circuit = PoE::new(point.x, point.y, exp);
            let _ = Groth16::prove(&pk, circuit, &mut rng).unwrap();
        })
    });
}

criterion_group!(benches, poe);
criterion_main!(benches);
