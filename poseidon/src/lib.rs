use ark_bls12_381::Fr;
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};
use ark_sponge::{constraints::CryptographicSpongeVar, poseidon::constraints::PoseidonSpongeVar};
use once_cell::sync::Lazy;
use poseidon_paramgen::PoseidonParameters;
use utils::to_ark_sponge_parameters;

mod utils;
pub mod parameters {
    include!(concat!(env!("OUT_DIR"), "/parameters.rs"));
}

pub type CircuitField = ark_bls12_381::Fr;
pub type FpVar = ark_r1cs_std::fields::fp::FpVar<CircuitField>;

/// Parameters for the 1 to 1 instance of Poseidon
// TODO : use lazy to assign ark parameters
pub static RATE_1_PARAMETERS: Lazy<PoseidonParameters<Fr>> = Lazy::new(parameters::rate_1);

/// hashes one field value inside the circuit
pub fn one_to_one_hash(
    cs: ConstraintSystemRef<Fr>,
    domain_separator: &FpVar,
    value: FpVar,
) -> Result<FpVar, SynthesisError> {
    // TODO : omit this lazily instantiate sponge params instead
    let ark_parameters = to_ark_sponge_parameters(RATE_1_PARAMETERS.clone());
    let mut state: PoseidonSpongeVar<Fr> = PoseidonSpongeVar::new(cs, &ark_parameters);

    state.absorb(&vec![domain_separator, &value])?;
    let result = state.squeeze_field_elements(1)?;
    Ok(result[0].clone())
}
