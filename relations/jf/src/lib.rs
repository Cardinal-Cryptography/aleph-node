use jf_plonk::{
    errors::PlonkError,
    proof_system::{
        structs::{Proof, ProvingKey, UniversalSrs, VerifyingKey},
        PlonkKzgSnark, UniversalSNARK,
    },
};
use jf_relation::PlonkCircuit;
use rand_core::{CryptoRng, RngCore};

pub mod deposit;
pub mod shielder_types;

pub type PlonkResult<T> = Result<T, PlonkError>;
pub type Curve = ark_bls12_381::Bls12_381;
pub type CircuitField = ark_bls12_381::Fr;

pub fn generate_srs<R: CryptoRng + RngCore>(
    max_degree: usize,
    rng: &mut R,
) -> PlonkResult<UniversalSrs<Curve>> {
    <PlonkKzgSnark<Curve> as UniversalSNARK<Curve>>::universal_setup_for_testing(max_degree, rng)
}

pub trait Relation: Default {
    type PublicInput;
    type PrivateInput;

    fn new(public_input: Self::PublicInput, private_input: Self::PrivateInput)
        -> PlonkResult<Self>;

    fn generate_circuit(&self) -> PlonkResult<PlonkCircuit<CircuitField>>;

    fn generate_keys(
        srs: &UniversalSrs<Curve>,
    ) -> PlonkResult<(ProvingKey<Curve>, VerifyingKey<Curve>)> {
        PlonkKzgSnark::<Curve>::preprocess(&srs, &Self::default().generate_circuit()?)
    }

    fn prove<R: CryptoRng + RngCore>(
        &self,
        pk: &ProvingKey<Curve>,
        rng: &mut R,
    ) -> PlonkResult<Proof<Curve>> {
        PlonkKzgSnark::<Curve>::prove(rng, &self.generate_circuit()?, pk, None)
    }

    fn process_public_input(
        &self,
        public_input: Self::PublicInput,
    ) -> PlonkResult<Vec<CircuitField>>;
}
