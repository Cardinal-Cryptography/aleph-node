//! A set of mock structs to test the aggregator whose purpose is to
//! typecheck and not much more: the implementations here don't need to
//! actually do anything, as the aggregator doesn't really rely on any of their
//! properties (they are used inside `aleph_bft`).

use crate::aggregation::{
    multicast::{HasSignature, Multisigned},
    SignableHash,
};
use aleph_bft::{Index, KeyBox, MultiKeychain, NodeCount, NodeIndex, PartialMultisignature};
use codec::{Decode, Encode};
use std::{fmt::Debug, hash::Hash as StdHash};

pub type THash = substrate_test_runtime::Hash;

// a very arbitrary seed for the signatures
const MAGIC_NUMBER: u32 = 42;

#[derive(Copy, Clone, Debug, Eq, StdHash, PartialEq, Decode, Encode)]
pub struct TestSignature(u32);

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TestMultisignature(Vec<TestSignature>);

impl TestMultisignature {
    fn generate() -> Self {
        TestMultisignature(vec![TestSignature(MAGIC_NUMBER)])
    }
}

impl PartialMultisignature for TestMultisignature {
    type Signature = TestSignature;

    fn add_signature(self, _signature: &TestSignature, _index: NodeIndex) -> Self {
        self // never used by the mocks
    }
}

impl HasSignature<TestMultiKeychain> for TestMultisignature {
    fn signature(&self) -> TestMultisignature {
        TestMultisignature::generate()
    }
}

#[derive(Clone, Copy, Eq, Debug, PartialEq)]
pub struct TestMultiKeychain {
    node_count: usize,
    node_index: usize,
}

#[async_trait::async_trait]
impl KeyBox for TestMultiKeychain {
    type Signature = TestSignature;

    fn node_count(&self) -> NodeCount {
        NodeCount(self.node_count)
    }

    async fn sign(&self, _msg: &[u8]) -> Self::Signature {
        return TestSignature(MAGIC_NUMBER);
    }

    fn verify(&self, _msg: &[u8], _sgn: &Self::Signature, _index: NodeIndex) -> bool {
        true
    }
}

impl Index for TestMultiKeychain {
    fn index(&self) -> NodeIndex {
        NodeIndex(self.node_index)
    }
}

impl MultiKeychain for TestMultiKeychain {
    type PartialMultisignature = TestMultisignature;

    fn from_signature(
        &self,
        _signature: &TestSignature,
        _index: NodeIndex,
    ) -> Self::PartialMultisignature {
        TestMultisignature::generate()
    }

    fn is_complete(&self, _msg: &[u8], _partial: &Self::PartialMultisignature) -> bool {
        true
    }
}

pub struct TestMultisigned {
    signable: SignableHash<THash>,
}

impl TestMultisigned {
    pub fn new(hash: THash) -> Self {
        TestMultisigned {
            signable: SignableHash::new(hash),
        }
    }
}

impl Multisigned<'_, SignableHash<THash>, TestMultiKeychain> for TestMultisigned {
    type Result = TestMultisignature;

    fn as_signable(&self) -> &SignableHash<THash> {
        &self.signable
    }

    fn into_unchecked(self) -> Self::Result {
        TestMultisignature::generate()
    }
}
