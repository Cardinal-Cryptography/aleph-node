//! A set of abstractions for dealing with `ReliableMulticast` in a more testable
//! and modular way.
//!
//! We expose the following traits:
//! - `Multicast`: mimicking the interface of `aleph_bft::ReliableMulticast`
//! - `Multisigned`: abstracting away the `aleph_bft::Multisigned` capabilities as a trait.
//!
//! For use in production code, we provide wrappers for the BFT code pieces:
//! - `RMCWrapper` that simply dispatches all calls to its internal `ReliableMulticast` instance
//! - `MultisignedWrapper` that does the same for `Multisigned`.

use crate::{aggregator::SignableHash, crypto::Signature};
use aleph_bft::rmc::ReliableMulticast;
use aleph_bft::{
    rmc::Message, MultiKeychain, Multisigned as MultisignedStruct, SignatureSet, UncheckedSigned,
};
use codec::Codec;
use sp_runtime::traits::Block;
use std::fmt::Debug;

/// A convenience trait for gathering all of the desired hash characteristics.
pub trait Hash: AsRef<[u8]> + std::hash::Hash + Eq + Clone + Codec + Debug + Send + Sync {}

impl<T: AsRef<[u8]> + std::hash::Hash + Eq + Clone + Codec + Debug + Send + Sync> Hash for T {}

pub type NetworkData<B> =
    Message<SignableHash<<B as Block>::Hash>, Signature, SignatureSet<Signature>>;

/// Anything that exposes the same interface as `aleph_bft::Multisigned` struct.
/// In production code this will be the `MultisignedWrapper`, containing an instance
/// of `aleph_bft::Multisigned` (here imported as `MultisignedStruct` for clarity).
pub trait Multisigned<'a, H: Hash, MK: MultiKeychain> {
    fn as_signable(&self) -> &SignableHash<H>;
    fn into_unchecked(self) -> UncheckedSigned<SignableHash<H>, MK::PartialMultisignature>;
}

pub struct MultisignedWrapper<'a, H: Hash, MK: MultiKeychain> {
    inner: MultisignedStruct<'a, SignableHash<H>, MK>,
}

/// A wrapper for `aleph_bft::Multisigned` that forwards all calls to its underlying instance.
impl<'a, H: Hash, MK: MultiKeychain> MultisignedWrapper<'a, H, MK> {
    pub fn wrap(inner: MultisignedStruct<'a, SignableHash<H>, MK>) -> Self {
        MultisignedWrapper { inner }
    }
}

impl<'a, H: Hash, MK: MultiKeychain> Multisigned<'a, H, MK> for MultisignedWrapper<'a, H, MK> {
    fn as_signable(&self) -> &SignableHash<H> {
        self.inner.as_signable()
    }

    fn into_unchecked(self) -> UncheckedSigned<SignableHash<H>, MK::PartialMultisignature> {
        self.inner.into_unchecked()
    }
}

/// Anything that exposes the same interface as `aleph_bft::ReliableMulticast`.
/// In production code this will most likely be the `RMCWrapper`, containing an instance
/// of `aleph_bft::ReliableMulticast`.
///
/// The trait defines an associated type: `Signed`. For `ReliableMulticast`, this is simply
/// `aleph_bft::Multisigned` but the mocks are free to use the simplest matching implementation.
#[async_trait::async_trait]
pub trait Multicast<H: Hash>: Send + Sync {
    type Signed;
    async fn start_multicast(&mut self, hash: SignableHash<H>);
    fn get_multisigned(&self, hash: &SignableHash<H>) -> Option<Self::Signed>;
    async fn next_multisigned_hash(&mut self) -> Self::Signed;
}

/// A wrapper for `aleph_bft::ReliableMulticast` that forwards all calls to its underlying instance.
pub struct RMCWrapper<'a, H: Hash, MK: MultiKeychain> {
    rmc: ReliableMulticast<'a, SignableHash<H>, MK>,
}

impl<'a, H: Hash, MK: MultiKeychain> RMCWrapper<'a, H, MK> {
    pub fn wrap(rmc: ReliableMulticast<'a, SignableHash<H>, MK>) -> Self {
        Self { rmc }
    }
}

#[async_trait::async_trait]
impl<'a, H: Hash, MK: MultiKeychain> Multicast<H> for RMCWrapper<'a, H, MK> {
    type Signed = MultisignedWrapper<'a, H, MK>;

    async fn start_multicast(&mut self, hash: SignableHash<H>) {
        self.rmc.start_rmc(hash).await;
    }

    fn get_multisigned(&self, hash: &SignableHash<H>) -> Option<MultisignedWrapper<'a, H, MK>> {
        let inner = self.rmc.get_multisigned(hash)?;
        Some(MultisignedWrapper::wrap(inner))
    }

    async fn next_multisigned_hash(&mut self) -> MultisignedWrapper<'a, H, MK> {
        MultisignedWrapper::wrap(self.rmc.next_multisigned_hash().await)
    }
}
