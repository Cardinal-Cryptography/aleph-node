use codec::Encode;

pub type Version = u16;
pub type ByteCount = u16;

/// A message that is under versioning.
///
/// There are some message types for which we need to provide compatibility between versions. By
/// [ADR 40], this is achieved by creating a type that looks like this:
///
/// ```ignore
/// enum VersionedFoo {
///     Other(Version, Vec<u8>),
///     V1(FooV1),
///     V2(FooV2),
///     ...
///     VN(Foo),
/// }
/// ```
///
/// In this example, `VersionedFoo` is the _versioned type_, and `Foo` can be said to be _under
/// versioning_.
///
/// This trait provides conversions from a type under versioning and its corresponding versioned
/// type ([`into_versioned`]), and the reverse ([`from_versioned`]).
///
/// In practice, many versioned messages only recognize a few versions (e.g. the current version and
/// the single version that preceded it), not every historical version.
///
/// [ADR 40]: https://www.notion.so/cardinalcryptography/Message-Compatibility-Quick-solution-ec47e2c4d2894a0387eabf26fcbf0115
pub trait MessageVersioning: Clone {
    type Versioned;

    fn into_versioned(self) -> Self::Versioned;

    fn from_versioned(_: Self::Versioned) -> Result<Self, UnrecognizedVersionError>;
}

pub struct UnrecognizedVersionError {
    pub version: Version,
    pub value: Vec<u8>,
}

pub fn encode_with_version(version: Version, mut payload: Vec<u8>) -> Vec<u8> {
    let mut result = version.encode();
    // This will produce rubbish if we ever try encodings that have more than u16::MAX bytes. We
    // expect this won't happen, since we will switch to proper multisignatures before proofs get
    // that big.
    let num_bytes = payload.len() as ByteCount;
    result.append(&mut num_bytes.encode());
    result.append(&mut payload);
    result
}
