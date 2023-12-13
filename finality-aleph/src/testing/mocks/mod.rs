use std::fmt::{Display, Error as FmtError, Formatter};

pub use acceptance_policy::AcceptancePolicy;
pub use block_finalizer::MockedBlockFinalizer;
pub use client::{Backend, TestClient, TestClientBuilder, TestClientBuilderExt};
pub use proposal::{
    aleph_data_from_blocks, aleph_data_from_headers, unvalidated_proposal_from_headers,
};
use sp_runtime::traits::BlakeTwo256;

use crate::block::{mock::MockHeader, EquivocationProof, HeaderVerifier, VerifiedHeader};

type Hashing = BlakeTwo256;
pub type THash = substrate_test_runtime::Hash;

#[derive(Clone)]
pub struct TestVerifier;

pub struct TestEquivocationProof;

impl Display for TestEquivocationProof {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(f, "this should never get created")
    }
}

impl EquivocationProof for TestEquivocationProof {
    fn are_we_equivocating(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct TestVerificationError;

impl Display for TestVerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(f, "this should never get created")
    }
}

impl HeaderVerifier<MockHeader> for TestVerifier {
    type EquivocationProof = TestEquivocationProof;
    type Error = TestVerificationError;

    fn verify_header(
        &mut self,
        header: MockHeader,
        _just_created: bool,
    ) -> Result<VerifiedHeader<MockHeader, Self::EquivocationProof>, Self::Error> {
        Ok(VerifiedHeader {
            header,
            maybe_equivocation_proof: None,
        })
    }

    fn own_block(&self, _header: &MockHeader) -> bool {
        false
    }
}

mod acceptance_policy;
mod block_finalizer;
mod client;
mod proposal;
mod single_action_mock;
