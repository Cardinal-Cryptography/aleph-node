use ark_bls12_381::{Bls12_381, Fr};
use ark_serialize::CanonicalDeserialize;
use ark_snark::SNARK;
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Decode, Encode, TypeInfo)]
pub enum ProvingSystem {
    Groth16,
    Gm17,
}

pub(super) trait VerifyingSystem {
    type CircuitField: CanonicalDeserialize;
    type Proof: CanonicalDeserialize;
    type VerifyingKey: CanonicalDeserialize;

    fn verify(
        key: &Self::VerifyingKey,
        input: &Vec<Self::CircuitField>,
        proof: &Self::Proof,
    ) -> Result<bool, ()>;
}

pub(super) struct Groth16;
impl VerifyingSystem for Groth16 {
    type CircuitField = Fr;
    type Proof = ark_groth16::Proof<Bls12_381>;
    type VerifyingKey = ark_groth16::VerifyingKey<Bls12_381>;

    fn verify(
        key: &Self::VerifyingKey,
        input: &Vec<Self::CircuitField>,
        proof: &Self::Proof,
    ) -> Result<bool, ()> {
        ark_groth16::Groth16::verify(&key, &input, &proof).map_err(|_| ())
    }
}

pub(super) struct Gm17;
impl VerifyingSystem for Gm17 {
    type CircuitField = Fr;
    type Proof = ark_gm17::Proof<Bls12_381>;
    type VerifyingKey = ark_gm17::VerifyingKey<Bls12_381>;

    fn verify(
        key: &Self::VerifyingKey,
        input: &Vec<Self::CircuitField>,
        proof: &Self::Proof,
    ) -> Result<bool, ()> {
        ark_gm17::GM17::verify(&key, &input, &proof).map_err(|_| ())
    }
}
