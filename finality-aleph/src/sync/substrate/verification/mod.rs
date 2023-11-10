use std::{
    collections::hash_map::Entry,
    fmt::{Debug, Display, Error as FmtError, Formatter},
    sync::Arc,
};

use hex::ToHex;
use parity_scale_codec::Encode;
use sc_client_api::HeaderBackend;
use sc_consensus_aura::{find_pre_digest, standalone::PreDigestLookupError, CompatibleDigestItem};
use sp_consensus_aura::sr25519::{AuthorityPair, AuthoritySignature as AuraSignature};
use sp_consensus_slots::Slot;
use sp_core::{Pair, H256};
use sp_runtime::traits::{Header as SubstrateHeader, Zero};

use crate::{
    aleph_primitives::{
        AccountId, AuraId, AuthoritySignature, Block, BlockNumber, Header, MILLISECS_PER_BLOCK,
    },
    session_map::AuthorityProvider,
    sync::{
        substrate::{
            verification::{cache::CacheError, verifier::SessionVerificationError},
            InnerJustification, Justification,
        },
        EquivocationProof as EquivocationProofT, Header as HeaderT, VerifiedHeader, Verifier,
    },
};

mod cache;
mod verifier;

pub use cache::VerifierCache;
pub use verifier::SessionVerifier;

// How many slots in the future (according to the system time) can the verified header be.
// Must be non-negative. Chosen arbitrarily by timorl.
const HEADER_VERIFICATION_SLOT_OFFSET: u64 = 10;

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
    IncorrectGenesis,
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
            IncorrectGenesis => write!(f, "incorrect genesis header"),
            MissingSeal => write!(f, "missing seal"),
            IncorrectSeal => write!(f, "incorrect seal"),
            MissingAuthorityData => write!(f, "missing authority data"),
            IncorrectAuthority => write!(f, "incorrect authority"),
        }
    }
}

pub struct EquivocationProof {
    header_a: Header,
    header_b: Header,
    author: AuraId,
    account_id: Option<AccountId>,
    are_we_equivocating: bool,
}

impl EquivocationProofT for EquivocationProof {
    fn are_we_equivocating(&self) -> bool {
        self.are_we_equivocating
    }
}

impl Display for EquivocationProof {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        match &self.account_id {
            Some(account_id) => write!(
                f,
                "account ID: {}, author: 0x{}, first header: {}, second header {}",
                account_id,
                self.author.encode_hex::<String>(),
                self.header_a.id(),
                self.header_b.id()
            ),
            None => write!(
                f,
                "author: 0x{}, first header: {}, second header {}; check the account ID by hand",
                self.author.encode_hex::<String>(),
                self.header_a.id(),
                self.header_b.id()
            ),
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

impl<AP, FS> VerifierCache<AP, FS, Header>
where
    AP: AuthorityProvider,
    FS: FinalizationInfo,
{
    fn parse_aura_header(
        &mut self,
        header: &mut Header,
    ) -> Result<(Slot, AuraSignature, H256, AuraId, Option<AccountId>), HeaderVerificationError>
    {
        use HeaderVerificationError::*;
        let slot =
            find_pre_digest::<Block, AuthoritySignature>(header).map_err(PreDigestLookupError)?;

        // pop the seal BEFORE hashing
        let seal = header.digest_mut().pop().ok_or(MissingSeal)?;
        let sig = seal.as_aura_seal().ok_or(IncorrectSeal)?;

        let pre_hash = header.hash();
        // push the seal back
        header.digest_mut().push(seal);

        // Aura: authorities are stored in the parent block
        let parent_number = header.number() - 1;
        let authorities = self
            .get_aura_authorities(parent_number)
            .map_err(|_| MissingAuthorityData)?;
        // Aura: round robin
        let idx = *slot % (authorities.len() as u64);
        let (maybe_account_id, author) = authorities
            .get(idx as usize)
            .expect("idx < authorities.len()")
            .clone();

        Ok((slot, sig, pre_hash, author, maybe_account_id))
    }

    // This function assumes that:
    // 1. This is not a genesis header
    // 2. Headers are created by Aura.
    // 3. Slot number is calculated using the current system time.
    fn verify_aura_header(
        &mut self,
        slot: &Slot,
        sig: &AuraSignature,
        pre_hash: H256,
        author: &AuraId,
    ) -> Result<(), VerificationError> {
        use HeaderVerificationError::*;
        // Aura: slot number is calculated using the system time.
        // This code duplicates one of the parameters that we pass to Aura when starting the node!
        let slot_now = Slot::from_timestamp(
            sp_timestamp::Timestamp::current(),
            sp_consensus_slots::SlotDuration::from_millis(MILLISECS_PER_BLOCK),
        );
        if *slot > slot_now + HEADER_VERIFICATION_SLOT_OFFSET {
            return Err(VerificationError::HeaderVerification(HeaderTooNew(*slot)));
        }
        if !AuthorityPair::verify(sig, pre_hash.as_ref(), author) {
            return Err(VerificationError::HeaderVerification(IncorrectAuthority));
        }
        Ok(())
    }

    // This function assumes that:
    // 1. This is not a genesis header
    // 2. Headers are created by Aura.
    fn check_for_equivocation(
        &mut self,
        header: &Header,
        slot: Slot,
        author: AuraId,
        maybe_account_id: Option<AccountId>,
        just_created: bool,
    ) -> Result<Option<EquivocationProof>, VerificationError> {
        match self.equivocation_cache.entry(slot.into()) {
            Entry::Occupied(occupied) => {
                let (cached_header, certainly_own) = occupied.into_mut();
                if cached_header == header {
                    *certainly_own |= just_created;
                    return Ok(None);
                }
                Ok(Some(EquivocationProof {
                    header_a: cached_header.clone(),
                    header_b: header.clone(),
                    are_we_equivocating: *certainly_own || just_created,
                    account_id: maybe_account_id,
                    author,
                }))
            }
            Entry::Vacant(vacant) => {
                vacant.insert((header.clone(), just_created));
                Ok(None)
            }
        }
    }
}

impl<AP, FS> Verifier<Justification> for VerifierCache<AP, FS, Header>
where
    AP: AuthorityProvider,
    FS: FinalizationInfo,
{
    type EquivocationProof = EquivocationProof;
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

    fn verify_header(
        &mut self,
        mut header: Header,
        just_created: bool,
    ) -> Result<VerifiedHeader<Justification, Self::EquivocationProof>, Self::Error> {
        // compare genesis header directly to the one we know
        if header.number().is_zero() {
            return match &header == self.genesis_header() {
                true => Ok(VerifiedHeader {
                    header,
                    maybe_equivocation_proof: None,
                }),
                false => Err(VerificationError::HeaderVerification(
                    HeaderVerificationError::IncorrectGenesis,
                )),
            };
        }
        let (slot, sig, pre_hash, author, maybe_account_id) =
            self.parse_aura_header(&mut header)
                .map_err(VerificationError::HeaderVerification)?;
        self.verify_aura_header(&slot, &sig, pre_hash, &author)?;
        let maybe_equivocation_proof =
            self.check_for_equivocation(&header, slot, author, maybe_account_id, just_created)?;
        Ok(VerifiedHeader {
            header,
            maybe_equivocation_proof,
        })
    }
}
