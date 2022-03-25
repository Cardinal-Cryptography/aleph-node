//! A set of mock structs to test the aggregator whose purpose is to
//! typecheck and not much more: the implementations here don't need to
//! actually do anything, as the aggregator doesn't really rely on any of their
//! properties (they are used inside `aleph_bft`).

use codec::{Decode, Encode};
use std::{fmt::Debug, hash::Hash as StdHash};
pub use substrate_test_runtime::Hash as THash;

// a very arbitrary seed for the signatures
const MAGIC_NUMBER: u32 = 42;

#[derive(Copy, Clone, Debug, Eq, StdHash, PartialEq, Decode, Encode)]
pub struct TestSignature(u32);

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TestMultisignature(Vec<TestSignature>);

impl TestMultisignature {
    pub fn generate() -> Self {
        TestMultisignature(vec![TestSignature(MAGIC_NUMBER)])
    }
}
