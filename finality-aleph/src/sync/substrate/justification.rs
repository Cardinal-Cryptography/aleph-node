use std::fmt::{Display, Error as FmtError, Formatter};

use aleph_primitives::{BlockNumber, SessionAuthorityData};
use codec::Encode;
use log::warn;
use sp_runtime::{
    traits::{Block, Header as SubstrateHeader},
    RuntimeAppPublic,
};

use crate::{
    crypto::AuthorityVerifier,
    justification::{AlephJustification, Verifier as LegacyVerifier},
    session::session_id_from_num,
    session_map::ReadOnlySessionMap,
    sync::{Justification as JustificationT, Verifier},
    AuthorityId, SessionPeriod,
};

/// A justification, including the related header.
#[derive(Clone)]
pub struct Justification<H: SubstrateHeader<Number = BlockNumber>> {
    header: H,
    raw_justification: AlephJustification,
}

impl<H: SubstrateHeader<Number = BlockNumber>> JustificationT for Justification<H> {
    type Header = H;
    type Unverified = Self;

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn into_unverified(self) -> Self::Unverified {
        self
    }
}

/// A justification verifier within a single session.
pub struct SessionVerifier {
    authority_verifier: AuthorityVerifier,
    emergency_signer: Option<AuthorityId>,
}

impl From<SessionAuthorityData> for SessionVerifier {
    fn from(authority_data: SessionAuthorityData) -> Self {
        SessionVerifier {
            authority_verifier: AuthorityVerifier::new(authority_data.authorities().to_vec()),
            emergency_signer: authority_data.emergency_finalizer().clone(),
        }
    }
}

/// Ways in which a justification can be wrong.
pub enum SessionVerificationError {
    BadMultisignature,
    BadEmergencySignature,
    NoEmergencySigner,
}

impl Display for SessionVerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use SessionVerificationError::*;
        match self {
            BadMultisignature => write!(f, "bad multisignature"),
            BadEmergencySignature => write!(f, "bad emergency signature"),
            NoEmergencySigner => write!(f, "no emergency signer defined"),
        }
    }
}

impl SessionVerifier {
    pub fn verify_bytes(
        &self,
        justification: &AlephJustification,
        bytes: Vec<u8>,
    ) -> Result<(), SessionVerificationError> {
        use AlephJustification::*;
        use SessionVerificationError::*;
        match justification {
            CommitteeMultisignature(multisignature) => {
                match self.authority_verifier.is_complete(&bytes, multisignature) {
                    true => Ok(()),
                    false => Err(BadMultisignature),
                }
            }
            EmergencySignature(signature) => match self
                .emergency_signer
                .as_ref()
                .ok_or(NoEmergencySigner)?
                .verify(&bytes, signature)
            {
                true => Ok(()),
                false => Err(BadEmergencySignature),
            },
        }
    }
}

// This shouldn't be necessary after we remove the legacy justification sync. Then we can also
// rewrite the implementation above and make it simpler.
impl<B: Block> LegacyVerifier<B> for SessionVerifier {
    fn verify(&self, justification: &AlephJustification, hash: B::Hash) -> bool {
        match self.verify_bytes(justification, hash.encode()) {
            Ok(()) => true,
            Err(e) => {
                warn!(target: "aleph-justification", "Bad justification for block {:?}: {}", hash, e);
                false
            }
        }
    }
}

/// Ways in which a justification can fail verification.
pub enum VerificationError {
    UnknownSession,
    Session(SessionVerificationError),
}

impl From<SessionVerificationError> for VerificationError {
    fn from(e: SessionVerificationError) -> Self {
        VerificationError::Session(e)
    }
}

impl Display for VerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use VerificationError::*;
        match self {
            UnknownSession => write!(f, "justification from unknown session"),
            Session(e) => write!(f, "{}", e),
        }
    }
}

/// A verifier working for all sessions, although if the session is too new or ancient it will fail
/// verification -- this is by design.
pub struct FullVerifier {
    sessions: ReadOnlySessionMap,
    period: SessionPeriod,
}

impl FullVerifier {
    fn session_verifier<H: SubstrateHeader<Number = BlockNumber>>(
        &self,
        header: &H,
    ) -> Result<SessionVerifier, VerificationError> {
        let session_id = session_id_from_num(*header.number(), self.period);
        self.sessions
            .get(session_id)
            .map(|authority_data| authority_data.into())
            .ok_or(VerificationError::UnknownSession)
    }
}

impl<H: SubstrateHeader<Number = BlockNumber>> Verifier<Justification<H>> for FullVerifier {
    type Error = VerificationError;

    fn verify(&self, justification: Justification<H>) -> Result<Justification<H>, Self::Error> {
        let header = &justification.header;
        let verifier = self.session_verifier(header)?;
        verifier.verify_bytes(&justification.raw_justification, header.hash().encode())?;
        Ok(justification)
    }
}
