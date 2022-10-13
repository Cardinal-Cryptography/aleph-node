mod common;
mod crypto;
mod current;
mod legacy;
mod network;
mod traits;
mod types;

use std::fmt::Debug;

use aleph_bft_crypto::{PartialMultisignature, Signature};
use codec::{Decode, Encode};
pub use crypto::Keychain;
pub use current::{
    create_aleph_config as current_create_aleph_config, run_member as run_current_member,
};
pub use legacy::{
    create_aleph_config as legacy_create_aleph_config, run_member as run_legacy_member,
};
pub use network::{CurrentNetworkData, LegacyNetworkData, NetworkWrapper};
pub use traits::{Hash, SpawnHandle, SpawnHandleT, Wrapper as HashWrapper};
pub use types::{NodeCount, NodeIndex, Recipient};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Encode, Decode)]
pub struct SignatureSet<Signature>(pub aleph_bft_crypto::SignatureSet<Signature>);

impl<S: Clone> SignatureSet<S> {
    pub fn size(&self) -> NodeCount {
        self.0.size().into()
    }

    pub fn with_size(len: NodeCount) -> Self {
        SignatureSet(legacy_aleph_bft::SignatureSet::with_size(len.into()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeIndex, &S)> {
        self.0.iter().map(|(idx, s)| (idx.into(), s))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeIndex, &mut S)> {
        self.0.iter_mut().map(|(idx, s)| (idx.into(), s))
    }

    pub fn into_iter(self) -> impl Iterator<Item = (NodeIndex, S)>
    where
        S: 'static,
    {
        self.0.into_iter().map(|(idx, s)| (idx.into(), s))
    }

    pub fn add_signature(self, signature: &S, index: NodeIndex) -> Self
    where
        S: Signature,
    {
        SignatureSet(self.0.add_signature(signature, index.into()))
    }
}

impl<S: legacy_aleph_bft::Signature> legacy_aleph_bft::PartialMultisignature for SignatureSet<S> {
    type Signature = S;

    fn add_signature(
        self,
        signature: &Self::Signature,
        index: legacy_aleph_bft::NodeIndex,
    ) -> Self {
        SignatureSet::add_signature(self, signature, index.into())
    }
}

impl<S: legacy_aleph_bft::Signature> current_aleph_bft::PartialMultisignature for SignatureSet<S> {
    type Signature = S;

    fn add_signature(
        self,
        signature: &Self::Signature,
        index: current_aleph_bft::NodeIndex,
    ) -> Self {
        SignatureSet::add_signature(self, signature, index.into())
    }
}
