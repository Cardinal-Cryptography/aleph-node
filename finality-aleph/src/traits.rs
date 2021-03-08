use codec::Codec;
use sp_runtime::traits::{
    MaybeDisplay, MaybeMallocSizeOf, MaybeSerializeDeserialize, Member, SimpleBitOps,
};
use std::fmt::Debug;

pub use sp_runtime::traits::{Block as SubstrateBlock, Header as SubstrateHeader};

pub trait SubstrateHeaderExt {
    type BlockHash: Member
        + MaybeSerializeDeserialize
        + Debug
        + std::hash::Hash
        + Ord
        + Copy
        + MaybeDisplay
        + Default
        + SimpleBitOps
        + Codec
        + AsRef<[u8]>
        + AsMut<[u8]>
        + MaybeMallocSizeOf;
}

pub trait Header: SubstrateHeader + SubstrateHeaderExt {}

pub trait SubstrateBlockExt {
    type Header: SubstrateHeaderExt;

    type BlockHash: Member
        + MaybeSerializeDeserialize
        + Debug
        + std::hash::Hash
        + Ord
        + Copy
        + MaybeDisplay
        + Default
        + SimpleBitOps
        + Codec
        + AsRef<[u8]>
        + AsMut<[u8]>
        + MaybeMallocSizeOf;
}

pub trait Block: SubstrateBlock + SubstrateBlockExt {}
