use std::{
    env, fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use ark_bls12_381::{Fr, FrParameters};
use ark_ff::fields::FpParameters;
use poseidon_paramgen::poseidon_build;

fn main() {
    let security_level = 128;
    // t = arity + 1, so t=2 is a 1:1 hash, t=3 is a 2:1 hash etc
    // see https://spec.filecoin.io/#section-algorithms.crypto.poseidon.filecoins-poseidon-instances for similar specification used by Filecoin
    let t_values = vec![2, 3];

    // Fr = Fp256
    let params_codegen =
        poseidon_build::compile::<Fr>(security_level, t_values, FrParameters::MODULUS, true);

    let output_directory: PathBuf =
        PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR environmental variable must be set"))
            .join("parameters.rs");

    let fh = fs::File::create(output_directory).expect("can't create source file");

    let mut f = BufWriter::new(fh);

    f.write_all(params_codegen.as_bytes())
        .expect("can write parameters to file");
}
