use ark_bls12_381::Fr;
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};
use ark_sponge::{constraints::CryptographicSpongeVar, poseidon::constraints::PoseidonSpongeVar};
use parameters::RATE_1_PARAMETERS;

mod parameters;

pub type CircuitField = ark_bls12_381::Fr;
pub type FpVar = ark_r1cs_std::fields::fp::FpVar<CircuitField>;

/// hashes one field value inside the circuit
pub fn one_to_one_hash(
    cs: ConstraintSystemRef<Fr>,
    domain_separator: &FpVar,
    value: FpVar,
) -> Result<FpVar, SynthesisError> {
    let mut state: PoseidonSpongeVar<Fr> = PoseidonSpongeVar::new(cs, &RATE_1_PARAMETERS);
    state.absorb(&vec![domain_separator, &value])?;
    let result = state.squeeze_field_elements(1)?;
    Ok(result[0].clone())
}
