use jf_plonk::{
    errors::PlonkError,
    proof_system::{
        structs::{Proof, ProvingKey, UniversalSrs, VerifyingKey},
        PlonkKzgSnark, UniversalSNARK,
    },
    transcript::StandardTranscript,
};
use jf_relation::PlonkCircuit;
use rand_core::{CryptoRng, RngCore};

pub mod deposit;
pub mod shielder_types;

pub type PlonkResult<T> = Result<T, PlonkError>;
pub type Curve = ark_bls12_381::Bls12_381;
pub type CircuitField = ark_bls12_381::Fr;

#[cfg(any(test, feature = "test-srs"))]
pub fn generate_srs<R: CryptoRng + RngCore>(
    max_degree: usize,
    rng: &mut R,
) -> PlonkResult<UniversalSrs<Curve>> {
    let srs = PlonkKzgSnark::<Curve>::universal_setup_for_testing(max_degree, rng).unwrap();
    Ok(srs)
}

pub trait Relation: Default {
    type PublicInput: Marshall;
    type PrivateInput;

    fn new(public_input: Self::PublicInput, private_input: Self::PrivateInput) -> Self;

    fn generate_subcircuit(&self, circuit: &mut PlonkCircuit<CircuitField>) -> PlonkResult<()>;

    fn generate_circuit(&self) -> PlonkResult<PlonkCircuit<CircuitField>> {
        let mut circuit = PlonkCircuit::<CircuitField>::new_turbo_plonk();
        self.generate_subcircuit(&mut circuit)?;
        circuit.finalize_for_arithmetization()?;
        Ok(circuit)
    }

    fn generate_keys(
        srs: &UniversalSrs<Curve>,
    ) -> PlonkResult<(ProvingKey<Curve>, VerifyingKey<Curve>)> {
        PlonkKzgSnark::<Curve>::preprocess(srs, &Self::default().generate_circuit()?)
    }

    fn prove<R: CryptoRng + RngCore>(
        &self,
        pk: &ProvingKey<Curve>,
        rng: &mut R,
    ) -> PlonkResult<Proof<Curve>> {
        PlonkKzgSnark::<Curve>::prove::<_, _, StandardTranscript>(
            rng,
            &self.generate_circuit()?,
            pk,
            None,
        )
    }
}

pub trait Marshall {
    fn marshall(&self) -> Vec<CircuitField>;
}
