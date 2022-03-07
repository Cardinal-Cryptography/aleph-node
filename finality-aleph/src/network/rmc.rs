use crate::{aggregator::SignableHash, crypto::Signature};
use aleph_bft::rmc::ReliableMulticast;
use aleph_bft::{
    rmc::Message, MultiKeychain, Multisigned as MultisignedStruct, Signable, SignatureSet,
    UncheckedSigned,
};
use sp_runtime::traits::Block;
use std::fmt::Debug;

pub trait Hash: Signable + std::hash::Hash + Eq + Clone + Debug + Send + Sync {}

impl<T: Signable + std::hash::Hash + Eq + Clone + Debug + Send + Sync> Hash for T {}

pub type NetworkData<B> =
    Message<SignableHash<<B as Block>::Hash>, Signature, SignatureSet<Signature>>;

pub trait Multisigned<'a, H: Hash, MK: MultiKeychain> {
    fn as_signable(&self) -> &H;
    fn into_unchecked(self) -> UncheckedSigned<H, MK::PartialMultisignature>;
}

pub struct MultisignedWrapper<'a, H: Hash, MK: MultiKeychain> {
    inner: MultisignedStruct<'a, H, MK>,
}

impl<'a, H: Hash, MK: MultiKeychain> MultisignedWrapper<'a, H, MK> {
    pub fn wrap(inner: MultisignedStruct<'a, H, MK>) -> Self {
        MultisignedWrapper { inner }
    }
}

impl<'a, H: Hash, MK: MultiKeychain> Multisigned<'a, H, MK> for MultisignedWrapper<'a, H, MK> {
    fn as_signable(&self) -> &H {
        self.inner.as_signable()
    }

    fn into_unchecked(self) -> UncheckedSigned<H, MK::PartialMultisignature> {
        self.inner.into_unchecked()
    }
}

#[async_trait::async_trait]
pub trait Multicast<H: Send + Sync>: Send + Sync {
    type Signed;
    async fn start_rmc(&mut self, hash: H);
    fn get_multisigned(&self, hash: &H) -> Option<Self::Signed>;
    async fn next_multisigned_hash(&mut self) -> Self::Signed;
}

pub struct RMCWrapper<'a, H: Hash, MK: MultiKeychain> {
    rmc: ReliableMulticast<'a, H, MK>,
}

impl<'a, H: Hash, MK: MultiKeychain> RMCWrapper<'a, H, MK> {
    pub fn wrap(rmc: ReliableMulticast<'a, H, MK>) -> Self {
        Self { rmc }
    }
}

#[async_trait::async_trait]
impl<'a, H: Hash, MK: MultiKeychain> Multicast<H> for RMCWrapper<'a, H, MK> {
    type Signed = MultisignedWrapper<'a, H, MK>;

    async fn start_rmc(&mut self, hash: H) {
        self.rmc.start_rmc(hash).await;
    }

    fn get_multisigned(&self, hash: &H) -> Option<MultisignedWrapper<'a, H, MK>> {
        let inner = self.rmc.get_multisigned(hash)?;
        Some(MultisignedWrapper::wrap(inner))
    }

    async fn next_multisigned_hash(&mut self) -> MultisignedWrapper<'a, H, MK> {
        MultisignedWrapper::wrap(self.rmc.next_multisigned_hash().await)
    }
}
