//! An interface that provides to the runtime a functionality of verifying halo2 SNARKs, together with related errors
//! and configuration.

use codec::{Decode, Encode};

/// Gathers errors that can happen during proof verification.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Encode, Decode)]
pub enum VerifierError {
    /// No verification key available under this identifier.
    UnknownVerificationKeyIdentifier,
    /// Couldn't deserialize public input.
    DeserializingPublicInputFailed,
    /// Couldn't deserialize verification key from storage.
    DeserializingVerificationKeyFailed,
    /// Verification procedure has failed. Proof still can be correct.
    VerificationFailed,
    /// Proof has been found as incorrect.
    IncorrectProof,
}

// Bring trait implementation helpers to the scope.
#[cfg(feature = "std")]
use implementation::*;

/// An interface that provides to the runtime a functionality of verifying halo2 SNARKs.
#[sp_runtime_interface::runtime_interface]
pub trait SnarkVerifier {
    /// Verify `proof` given `verifying_key`.
    fn verify(
        proof: &[u8],
        public_input: &[u8],
        verifying_key: &[u8],
    ) -> Result<(), VerifierError> {
        let instances = deserialize_public_input(public_input)?;
        let verifying_key = deserialize_verifying_key(verifying_key)?;

        let mut transcript = Blake2bRead::init(&proof[..]);
        let params = ParamsVerifierKZG::<Curve>::mock(10);

        verify_proof::<_, VerifierGWC<_>, _, _, _>(
            &params,
            &verifying_key,
            SingleStrategy::new(&params),
            &[&[&instances]],
            &mut transcript,
        )
        .map_err(|err| match err {
            Error::ConstraintSystemFailure => VerifierError::IncorrectProof,
            _ => {
                log::debug!("Failed to verify a proof: {err:?}");
                VerifierError::VerificationFailed
            }
        })
    }
}

// Reexport `verify` and `HostFunctions`, so that they are not imported like
// `aleph-runtime-interfaces::snark_verifier::snark_verifier::<>`.
pub use snark_verifier::verify;
#[cfg(feature = "std")]
pub use snark_verifier::HostFunctions;

// We move all imports, type aliases and auxiliary helpers for the interface (host-side) implementation in one place.
#[cfg(feature = "std")]
mod implementation {
    pub use halo2_proofs::{
        plonk::{verify_proof, Error, VerifyingKey},
        poly::kzg::{
            commitment::{KZGCommitmentScheme, ParamsVerifierKZG},
            multiopen::VerifierGWC,
            strategy::SingleStrategy,
        },
        standard_plonk::StandardPlonk,
        transcript::{Blake2bRead, TranscriptReadBuffer},
        SerdeFormat,
    };

    use crate::snark_verifier::VerifierError;

    pub type Curve = halo2_proofs::halo2curves::bn256::Bn256;
    pub type G1Affine = halo2_proofs::halo2curves::bn256::G1Affine;
    pub type Fr = halo2_proofs::halo2curves::bn256::Fr;

    pub fn deserialize_public_input(raw: &[u8]) -> Result<Vec<Fr>, VerifierError> {
        raw.chunks(32)
            .map(|bytes| {
                let bytes = bytes.try_into().map_err(|_| {
                    log::debug!("Public input length is not multiple of 32");
                    VerifierError::DeserializingPublicInputFailed
                })?;
                Option::from(Fr::from_bytes(bytes))
                    .ok_or(VerifierError::DeserializingPublicInputFailed)
            })
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn deserialize_verifying_key(key: &[u8]) -> Result<VerifyingKey<G1Affine>, VerifierError> {
        // We use `SerdeFormat::RawBytesUnchecked` here for performance reasons.
        VerifyingKey::from_bytes::<StandardPlonk>(key, SerdeFormat::RawBytesUnchecked).map_err(
            |err| {
                log::debug!("Failed to deserialize verification key: {err:?}");
                VerifierError::DeserializingVerificationKeyFailed
            },
        )
    }
}
