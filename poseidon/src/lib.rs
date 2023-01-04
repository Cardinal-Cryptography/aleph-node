mod parameters;

type CircuitField = ark_bls12_381::Fr;
type FpVar = ark_r1cs_std::fields::fp::FpVar<CircuitField>;

pub mod hash {
    use ark_bls12_381::Fr;
    use poseidon_permutation::Instance;

    use crate::parameters::RATE_1_PARAMETERS;
    /// hashes one field value, outputs a fixed length field value
    pub fn one_to_one_hash(domain_separator: &Fr, value: Fr) -> Fr {
        let parameters = RATE_1_PARAMETERS.clone();
        let mut state = Instance::new(&parameters);
        state.n_to_1_fixed_hash(vec![*domain_separator, value])
    }
}

pub mod r1cs {
    use ark_bls12_381::Fr;
    use ark_relations::r1cs::{ConstraintSystemRef, SynthesisError};
    use ark_sponge::{
        constraints::CryptographicSpongeVar, poseidon::constraints::PoseidonSpongeVar,
    };

    use super::FpVar;
    use crate::parameters::{to_ark_sponge_poseidon_parameters, RATE_1_PARAMETERS};

    /// hashes one field value inside the circuit    
    pub fn one_to_one_hash(
        cs: ConstraintSystemRef<Fr>,
        domain_separator: &FpVar,
        value: FpVar,
    ) -> Result<FpVar, SynthesisError> {
        let mut state: PoseidonSpongeVar<Fr> = PoseidonSpongeVar::new(
            cs,
            &to_ark_sponge_poseidon_parameters(RATE_1_PARAMETERS.clone()),
        );
        state.absorb(&vec![domain_separator, &value])?;
        let result = state.squeeze_field_elements(1)?;
        Ok(result[0].clone())
    }
}
