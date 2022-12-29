use ark_bls12_381::{Fq, FqParameters};
use ark_ff::FpParameters;
use poseidon_paramgen::poseidon_build;

fn main() {
    let security_level = 128;
    // t = arity + 1, so t=2 is a 1:1 hash
    // see https://spec.filecoin.io/#section-algorithms.crypto.poseidon.filecoins-poseidon-instances for similar specification used by Filecoin
    let t_values = vec![2, 3];

    let params_codegen =
        poseidon_build::compile::<Fq>(security_level, t_values, FqParameters::MODULUS, true);

    println!("@@@ {}", params_codegen);
}
