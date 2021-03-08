use crate::traits::{
    Block, Header, SubstrateBlock, SubstrateBlockExt, SubstrateHeader, SubstrateHeaderExt,
};
use codec::{Codec, Decode, Encode};
use parity_util_mem::MallocSizeOf;
use serde::{Deserialize, Serialize};
use sp_core::RuntimeDebug;
use sp_runtime::{
    traits::{Extrinsic, MaybeMallocSizeOf, MaybeSerialize, Member},
    Justification,
};

/// Abstraction over a substrate block.
#[derive(Debug, PartialEq, Eq, Clone, MallocSizeOf, Serialize, Deserialize, Encode, Decode)]
pub struct AlephBlock<H, E: MaybeSerialize> {
    /// The block header.
    pub header: H,
    /// The accompanying extrinsics.
    pub extrinsics: Vec<E>,
}

impl<H, E: MaybeSerialize> SubstrateBlock for AlephBlock<H, E>
where
    H: Header,
    E: Member + Codec + Extrinsic + MaybeMallocSizeOf,
{
    type Extrinsic = E;
    type Header = H;
    type Hash = <Self::Header as SubstrateHeader>::Hash;

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn extrinsics(&self) -> &[Self::Extrinsic] {
        &self.extrinsics[..]
    }

    fn deconstruct(self) -> (Self::Header, Vec<Self::Extrinsic>) {
        (self.header, self.extrinsics)
    }

    fn new(header: Self::Header, extrinsics: Vec<Self::Extrinsic>) -> Self {
        AlephBlock { header, extrinsics }
    }

    fn encode_from(header: &Self::Header, extrinsics: &[Self::Extrinsic]) -> Vec<u8> {
        (header, extrinsics).encode()
    }
}

impl<H, E: MaybeSerialize> SubstrateBlockExt for AlephBlock<H, E>
where
    H: Header,
    E: Member + Codec + Extrinsic + MaybeMallocSizeOf,
{
    type Header = H;
    type BlockHash = <Self::Header as SubstrateHeaderExt>::BlockHash;
}

impl<H, E: MaybeSerialize> Block for AlephBlock<H, E>
where
    H: Header,
    E: Member + Codec + Extrinsic + MaybeMallocSizeOf,
{
}

/// Abstraction over a substrate block and justification.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Serialize, Deserialize)]
pub struct SignedAlephBlock<B> {
    /// Full block.
    pub block: B,
    /// Block justification.
    pub justification: Option<Justification>,
}
