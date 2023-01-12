// RUN IT
// cargo bench
//

use ark_relations::r1cs::ConstraintSystem;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use poseidon::hash;
use relations::{CircuitField, ConstraintSynthesizer, PreimageRelation};

fn preimage(c: &mut Criterion) {
    c.bench_function("take_left_path", |f| {
        f.iter(|| {
            let preimage = CircuitField::from(17u64);
            let image = hash::one_to_one_hash(preimage);

            let circuit = PreimageRelation::with_full_input(preimage, image);

            let cs = ConstraintSystem::new_ref();
            circuit.generate_constraints(cs.clone()).unwrap();

            println!("preimage circuit size: {}", cs.num_constraints());
        })
    });
}

criterion_group!(benches, preimage);
criterion_main!(benches);
