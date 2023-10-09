use std::{
    fmt::{Debug, Display, Error as FmtError, Formatter},
    sync::Arc,
};

use log::info;
use parity_scale_codec::Encode;
use sc_client_api::HeaderBackend;
use sc_consensus_aura::{find_pre_digest, standalone::PreDigestLookupError, CompatibleDigestItem};
use sp_consensus_aura::sr25519::AuthorityPair;
use sp_consensus_slots::Slot;
use sp_core::Pair;
use sp_runtime::traits::{Header as SubstrateHeader, Zero};

use crate::{
    aleph_primitives::{AuthoritySignature, Block, BlockNumber, Header},
    session_map::AuthorityProvider,
    sync::{
        substrate::{
            verification::{cache::CacheError, verifier::SessionVerificationError},
            InnerJustification, Justification,
        },
        Verifier,
    },
};

mod cache;
mod verifier;

pub use cache::VerifierCache;
pub use verifier::SessionVerifier;

/// Supplies finalized number. Will be unified together with other traits we used in A0-1839.
pub trait FinalizationInfo {
    fn finalized_number(&self) -> BlockNumber;
}

/// Substrate specific implementation of `FinalizationInfo`
pub struct SubstrateFinalizationInfo<BE: HeaderBackend<Block>>(Arc<BE>);

impl<BE: HeaderBackend<Block>> SubstrateFinalizationInfo<BE> {
    pub fn new(client: Arc<BE>) -> Self {
        Self(client)
    }
}

impl<BE: HeaderBackend<Block>> FinalizationInfo for SubstrateFinalizationInfo<BE> {
    fn finalized_number(&self) -> BlockNumber {
        self.0.info().finalized_number
    }
}

#[derive(Debug)]
pub enum HeaderVerificationError {
    PreDigestLookupError(PreDigestLookupError),
    HeaderTooNew(Slot),
    MissingSeal,
    IncorrectSeal,
    MissingAuthorityData,
    IncorrectAuthority,
}

impl Display for HeaderVerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use HeaderVerificationError::*;
        match self {
            PreDigestLookupError(e) => write!(f, "pre digest lookup error, {e}"),
            HeaderTooNew(slot) => write!(f, "slot {slot} too far in the future"),
            MissingSeal => write!(f, "missing seal"),
            IncorrectSeal => write!(f, "incorrect seal"),
            MissingAuthorityData => write!(f, "missing authority data"),
            IncorrectAuthority => write!(f, "incorrect authority"),
        }
    }
}

#[derive(Debug)]
pub enum VerificationError {
    Verification(SessionVerificationError),
    Cache(CacheError),
    HeaderVerification(HeaderVerificationError),
}

impl From<SessionVerificationError> for VerificationError {
    fn from(e: SessionVerificationError) -> Self {
        VerificationError::Verification(e)
    }
}

impl From<CacheError> for VerificationError {
    fn from(e: CacheError) -> Self {
        VerificationError::Cache(e)
    }
}

impl Display for VerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use VerificationError::*;
        match self {
            Verification(e) => write!(f, "{e}"),
            Cache(e) => write!(f, "{e}"),
            HeaderVerification(e) => write!(f, "{e}"),
        }
    }
}

impl<AP, FS> Verifier<Justification> for VerifierCache<AP, FS, Header>
where
    AP: AuthorityProvider,
    FS: FinalizationInfo,
{
    type Error = VerificationError;

    fn verify_justification(
        &mut self,
        justification: Justification,
    ) -> Result<Justification, Self::Error> {
        let header = &justification.header;
        match &justification.inner_justification {
            InnerJustification::AlephJustification(aleph_justification) => {
                let verifier = self.get(*header.number())?;
                verifier.verify_bytes(aleph_justification, header.hash().encode())?;
                Ok(justification)
            }
            InnerJustification::Genesis => match header == self.genesis_header() {
                true => Ok(justification),
                false => Err(Self::Error::Cache(CacheError::BadGenesisHeader)),
            },
        }
    }

    fn verify_header(&mut self, header: Header) -> Result<Header, Self::Error> {
        use HeaderVerificationError::*;
        if header.number().is_zero() {
            return Ok(header);
        }
        let slot = find_pre_digest::<Block, AuthoritySignature>(&header)
            .map_err(|e| Self::Error::HeaderVerification(PreDigestLookupError(e)))?;
        let slot_now = Slot::from_timestamp(
            sp_timestamp::Timestamp::current(),
            sp_consensus_slots::SlotDuration::from_millis(1000),
        );
        if slot > slot_now + 10 {
            return Err(Self::Error::HeaderVerification(HeaderTooNew(slot)));
        }
        let seal = header
            .digest()
            .logs()
            .last()
            .ok_or(Self::Error::HeaderVerification(MissingSeal))?;
        let sig = seal
            .as_aura_seal()
            .ok_or(Self::Error::HeaderVerification(IncorrectSeal))?;
        let authorities = self
            .authorities(*header.parent_hash())
            .ok_or(Self::Error::HeaderVerification(MissingAuthorityData))?;
        let idx = *slot % (authorities.len() as u64);
        assert!(
            idx <= usize::MAX as u64,
            "It is impossible to have a vector with length beyond the address space; qed",
        );
        let author = authorities.get(idx as usize).expect(
            "authorities not empty; index constrained to list length;this is a valid index; qed",
        );

        info!("index index index {idx}");
        for author in authorities.iter() {
            let result = AuthorityPair::verify(&sig, header.hash().as_ref(), author);
            info!("{result}");
        }
        info!("done");

        if !AuthorityPair::verify(&sig, header.hash().as_ref(), author) {
            return Err(Self::Error::HeaderVerification(IncorrectAuthority));
        }
        Ok(header)
    }
}
