//! An interface that provides to the runtime a functionality of verifying halo2 SNARKs, together with related errors
//! and configuration.

use codec::{Decode, Encode};
#[cfg(feature = "std")]
use halo2_proofs::{
    plonk::{verify_proof, Error, VerifyingKey},
    poly::kzg::{
        commitment::{KZGCommitmentScheme, ParamsVerifierKZG},
        multiopen::VerifierGWC,
        strategy::SingleStrategy,
    },
    standard_plonk::StandardPlonk,
    transcript::{Blake2bRead, Challenge255, TranscriptReadBuffer},
    SerdeFormat,
};

/// Circuit curve.
#[cfg(feature = "std")]
pub type Curve = halo2_proofs::halo2curves::bn256::Bn256;
/// Curve (G1) point in its affine form.
#[cfg(feature = "std")]
pub type G1Affine = halo2_proofs::halo2curves::bn256::G1Affine;
/// The scalar field for circuits.
#[cfg(feature = "std")]
pub type Fr = halo2_proofs::halo2curves::bn256::Fr;

/// Gathers errors that can happen during proof verification.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Encode, Decode)]
pub enum VerifierError {
    /// No verification key available under this identifier.
    UnknownVerificationKeyIdentifier,
    /// Couldn't deserialize proof.
    DeserializingProofFailed,
    /// Couldn't deserialize public input.
    DeserializingPublicInputFailed,
    /// Couldn't deserialize verification key from storage.
    DeserializingVerificationKeyFailed,
    /// Verification procedure has failed. Proof still can be correct.
    VerificationFailed,
    /// Proof has been found as incorrect.
    IncorrectProof,
}

/// An interface that provides to the runtime a functionality of verifying halo2 SNARKs.
#[sp_runtime_interface::runtime_interface]
pub trait SnarkVerifier {
    /// Verify `proof` given `verifying_key`.
    fn verify(proof: &[u8], verifying_key: &[u8]) -> Result<(), VerifierError> {
        let instances: &[&[Fr]] = &[&[Fr::one()]];
        let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&proof[..]);
        let params = ParamsVerifierKZG::mock(10);
        let verifying_key = VerifyingKey::from_bytes::<StandardPlonk>(
            verifying_key,
            SerdeFormat::RawBytesUnchecked,
        )
        .map_err(|err| {
            log::debug!("Failed to deserialize verification key: {err:?}");
            VerifierError::DeserializingVerificationKeyFailed
        })?;

        verify_proof::<
            KZGCommitmentScheme<Curve>,
            VerifierGWC<'_, Curve>,
            Challenge255<G1Affine>,
            Blake2bRead<&[u8], G1Affine, Challenge255<G1Affine>>,
            SingleStrategy<'_, Curve>,
        >(
            &params,
            &verifying_key,
            SingleStrategy::new(&params),
            &[instances],
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
