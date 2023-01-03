use ark_bls12_381::Fq;
use ark_r1cs_std::fields::fp::FpVar;
use ark_relations::r1cs::SynthesisError;
use once_cell::sync::Lazy;
use poseidon_paramgen::PoseidonParameters;
use poseidon_permutation::Instance;

pub mod parameters {
    include!(concat!(env!("OUT_DIR"), "/parameters.rs"));
}

/// Parameters for the rate-1 instance of Poseidon.
pub static RATE_1_PARAMETERS: Lazy<PoseidonParameters<Fq>> = Lazy::new(parameters::rate_1);

pub type CircuitField = ark_bls12_381::Fr;
pub type CircuitVar = FpVar<CircuitField>;

pub fn one_to_one_hash(domain_separator: &Fq, value: Fq) -> Result<Fq, SynthesisError> {
    let mut state = Instance::new(&RATE_1_PARAMETERS);
    let hash = state.n_to_1_fixed_hash(vec![*domain_separator, value]);
    Ok(hash)
}
