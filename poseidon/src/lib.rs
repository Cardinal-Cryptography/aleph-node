use ark_bls12_381::Fr;
use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};
use ark_sponge::{
    constraints::CryptographicSpongeVar,
    poseidon::{constraints::PoseidonSpongeVar, PoseidonParameters as ArkPoseidonParameters},
};
use once_cell::sync::Lazy;
use poseidon_paramgen::{Alpha, PoseidonParameters};
use poseidon_permutation::Instance;

pub mod parameters {
    include!(concat!(env!("OUT_DIR"), "/parameters.rs"));
}

pub type CircuitField = ark_bls12_381::Fr;
pub type FpVar = ark_r1cs_std::fields::fp::FpVar<CircuitField>;

/// Parameters for the rate-1 instance of Poseidon
pub static RATE_1_PARAMETERS: Lazy<PoseidonParameters<Fr>> = Lazy::new(parameters::rate_1);

fn convert_to_ark_sponge_parameters(params: PoseidonParameters<Fr>) -> ArkPoseidonParameters<Fr> {
    let alpha = match params.alpha {
        Alpha::Exponent(exp) => exp as u64,
        Alpha::Inverse => panic!("ark-sponge does not allow inverse alpha"),
    };
    let capacity = 1;
    let rate = params.t - capacity;
    let full_rounds = params.rounds.full();
    let partial_rounds = params.rounds.partial();

    ArkPoseidonParameters {
        full_rounds,
        partial_rounds,
        alpha,
        ark: params.arc.into(),
        mds: params.mds.into(),
        rate,
        capacity,
    }
}

pub fn one_to_one_hash(
    cs: ConstraintSystemRef<Fr>,
    domain_separator: &FpVar,
    value: FpVar,
) -> Result<FpVar, SynthesisError> {
    let ark_parameters = convert_to_ark_sponge_parameters(RATE_1_PARAMETERS.clone());
    let mut state: PoseidonSpongeVar<Fr> = PoseidonSpongeVar::new(cs, &ark_parameters);

    state.absorb(&vec![domain_separator, &value])?;
    let result = state.squeeze_field_elements(1)?;
    Ok(result[0].clone())
}
