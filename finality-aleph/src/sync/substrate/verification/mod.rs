use std::{
    fmt::{Debug, Display, Error as FmtError, Formatter},
    sync::Arc,
};

use parity_scale_codec::Encode;
use sc_client_api::HeaderBackend;
use sc_consensus_aura::{find_pre_digest, CompatibleDigestItem};
use sp_core::Pair;
use sp_runtime::traits::Header as SubstrateHeader;

use crate::{
    aleph_primitives::{AuthorityPair, AuthoritySignature, Block, BlockNumber, Header},
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

#[derive(Debug, PartialEq, Eq)]
pub enum VerificationError {
    Verification(SessionVerificationError),
    Cache(CacheError),
    Aura,
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
            Aura => write!(f, "Aura error"),
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
        let slot =
            find_pre_digest::<Block, AuthoritySignature>(&header).map_err(|_| Self::Error::Aura)?;
        let slot_now = sp_consensus_slots::Slot::from_timestamp(
            sp_timestamp::Timestamp::current(),
            sp_consensus_slots::SlotDuration::from_millis(1000),
        );
        if slot > slot_now + 10 {
            return Err(Self::Error::Aura);
        }
        let seal = header.digest().logs().last().ok_or(Self::Error::Aura)?;
        let sig = seal.as_aura_seal().ok_or(Self::Error::Aura)?;
        let pre_hash = header.hash();
        let authority_data = self
            .authority_data(*header.number())
            .ok_or(Self::Error::Aura)?;
        let authorities = authority_data.authorities();
        let idx = *slot % (authorities.len() as u64);
        assert!(
            idx <= usize::MAX as u64,
            "It is impossible to have a vector with length beyond the address space; qed",
        );
        let author = authorities.get(idx as usize).expect(
            "authorities not empty; index constrained to list length;this is a valid index; qed",
        );
        if !AuthorityPair::verify(&sig, pre_hash.as_ref(), author) {
            return Err(Self::Error::Aura);
        }
        Ok(header)
    }
}
