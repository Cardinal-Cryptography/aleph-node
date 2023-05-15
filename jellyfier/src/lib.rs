use ark_bls12_381::{Bls12_381, Fr};
use ark_serialize::CanonicalDeserialize;
use jf_plonk::{
    errors::PlonkError,
    proof_system::{
        structs::{Proof, VerifyingKey},
        PlonkKzgSnark, UniversalSNARK,
    },
    transcript::StandardTranscript,
};
use sp_runtime_interface::pass_by::PassByEnum;

pub type Curve = Bls12_381;
pub type CircuitField = Fr;

#[derive(Copy, Clone, Eq, PartialEq, Debug, codec::Encode, codec::Decode, PassByEnum)]
pub enum VerificationError {
    WrongProof,
    DeserializationError,
    OtherError,
}

#[sp_runtime_interface::runtime_interface]
pub trait Jellyfier {
    fn verify_proof(
        vk: Vec<u8>,
        public_input: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(), VerificationError> {
        let vk: VerifyingKey<Curve> = CanonicalDeserialize::deserialize_compressed(&*vk)
            .map_err(|_| VerificationError::DeserializationError)?;
        let public_input: Vec<CircuitField> =
            CanonicalDeserialize::deserialize_compressed(&*public_input)
                .map_err(|_| VerificationError::DeserializationError)?;
        let proof: Proof<Curve> = CanonicalDeserialize::deserialize_compressed(&*proof)
            .map_err(|_| VerificationError::DeserializationError)?;

        PlonkKzgSnark::verify::<StandardTranscript>(&vk, &public_input, &proof, None).map_err(|e| {
            match e {
                PlonkError::WrongProof => VerificationError::WrongProof,
                _ => VerificationError::OtherError,
            }
        })
    }
}
