use jf_plonk::{
    errors::PlonkError,
    proof_system::{
        structs::{Proof, ProvingKey, UniversalSrs, VerifyingKey},
        PlonkKzgSnark, UniversalSNARK,
    },
};
use rand_core::{CryptoRng, RngCore};

pub mod deposit;

pub type PlonkResult<T> = Result<T, PlonkError>;
pub type Curve = ark_bls12_381::Bls12_381;
pub type CircuitField = ark_bls12_381::Fr;

pub fn generate_srs<R: CryptoRng + RngCore>(
    max_degree: usize,
    rng: &mut R,
) -> PlonkResult<UniversalSrs<Curve>> {
    <PlonkKzgSnark<Curve> as UniversalSNARK<Curve>>::universal_setup_for_testing(max_degree, rng)
}

pub trait Circuit {
    type PublicInput;
    type PrivateInput;

    fn generate_keys(
        &self,
        srs: &UniversalSrs<Curve>,
    ) -> PlonkResult<(ProvingKey<Curve>, VerifyingKey<Curve>)>;

    fn new(public_input: Self::PublicInput, private_input: Self::PrivateInput)
        -> PlonkResult<Self>;

    fn prove(&self, pk: &ProvingKey<Curve>) -> PlonkResult<Proof<Curve>>;

    fn process_public_input(
        &self,
        public_input: Self::PublicInput,
    ) -> PlonkResult<Vec<CircuitField>>;
}
