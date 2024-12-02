use std::fmt::Debug;

use aleph_bft_crypto::{PartialMultisignature, Signature};
use derive_more::{From, Into};
use parity_scale_codec::{Decode, Encode, Error, Input, Output};

/// The index of a node
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, From, Into)]
pub struct NodeIndex(pub usize);

impl Encode for NodeIndex {
    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        (self.0 as u64).encode_to(dest);
    }
}

impl Decode for NodeIndex {
    fn decode<I: Input>(value: &mut I) -> Result<Self, Error> {
        Ok(NodeIndex(u64::decode(value)? as usize))
    }
}

/// Node count. Right now it doubles as node weight in many places in the code, in the future we
/// might need a new type for that.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, From, Into)]
pub struct NodeCount(pub usize);

impl From<NodeCount> for current_aleph_bft::NodeCount {
    fn from(count: NodeCount) -> Self {
        current_aleph_bft::NodeCount(count.0)
    }
}
impl From<NodeCount> for legacy_aleph_bft::NodeCount {
    fn from(count: NodeCount) -> Self {
        legacy_aleph_bft::NodeCount(count.0)
    }
}

impl From<legacy_aleph_bft::NodeCount> for NodeCount {
    fn from(count: legacy_aleph_bft::NodeCount) -> Self {
        Self(count.0)
    }
}

impl From<current_aleph_bft::NodeCount> for NodeCount {
    fn from(count: current_aleph_bft::NodeCount) -> Self {
        Self(count.0)
    }
}

impl From<NodeIndex> for current_aleph_bft::NodeIndex {
    fn from(idx: NodeIndex) -> Self {
        current_aleph_bft::NodeIndex(idx.0)
    }
}

impl From<NodeIndex> for legacy_aleph_bft::NodeIndex {
    fn from(idx: NodeIndex) -> Self {
        legacy_aleph_bft::NodeIndex(idx.0)
    }
}

impl From<legacy_aleph_bft::NodeIndex> for NodeIndex {
    fn from(idx: legacy_aleph_bft::NodeIndex) -> Self {
        Self(idx.0)
    }
}

impl From<current_aleph_bft::NodeIndex> for NodeIndex {
    fn from(idx: current_aleph_bft::NodeIndex) -> Self {
        Self(idx.0)
    }
}

/// Wrapper for `SignatureSet` to be able to implement both legacy and current `PartialMultisignature` trait.
/// Inner `SignatureSet` is imported from `aleph_bft_crypto` with fixed version for compatibility reasons:
/// this is also used in the justification which already exist in our chain history and we
/// need to be careful with changing this.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Encode, Decode)]
pub struct SignatureSet<Signature>(pub aleph_bft_crypto::SignatureSet<Signature>);

impl<S: Clone> SignatureSet<S> {
    pub fn size(&self) -> NodeCount {
        self.0.size().into()
    }

    pub fn with_size(len: NodeCount) -> Self {
        SignatureSet(aleph_bft_crypto::SignatureSet::with_size(len.into()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeIndex, &S)> {
        self.0.iter().map(|(idx, s)| (idx.into(), s))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeIndex, &mut S)> {
        self.0.iter_mut().map(|(idx, s)| (idx.into(), s))
    }

    pub fn add_signature(self, signature: &S, index: NodeIndex) -> Self
    where
        S: Signature,
    {
        SignatureSet(self.0.add_signature(signature, index.into()))
    }
}

impl<S: 'static> IntoIterator for SignatureSet<S> {
    type Item = (NodeIndex, S);
    type IntoIter = Box<dyn Iterator<Item = (NodeIndex, S)>>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.0.into_iter().map(|(idx, s)| (idx.into(), s)))
    }
}

impl<S: Signature> legacy_aleph_bft::PartialMultisignature for SignatureSet<S> {
    type Signature = S;

    fn add_signature(
        self,
        signature: &Self::Signature,
        index: legacy_aleph_bft::NodeIndex,
    ) -> Self {
        SignatureSet::add_signature(self, signature, index.into())
    }
}

impl<S: Signature> current_aleph_bft::PartialMultisignature for SignatureSet<S> {
    type Signature = S;

    fn add_signature(
        self,
        signature: &Self::Signature,
        index: current_aleph_bft::NodeIndex,
    ) -> Self {
        SignatureSet::add_signature(self, signature, index.into())
    }
}
