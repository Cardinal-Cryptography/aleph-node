use std::{
    fmt::{Display, Error as FmtError, Formatter},
    mem::size_of,
};

use codec::{Decode, Encode, Error as CodecError, Input as CodecInput};

use crate::network::{
    manager::{DiscoveryMessage, NetworkData},
    Data, Multiaddress,
};

type Version = u16;
type ByteCount = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionedAuthentication<M: Multiaddress> {
    // Most likely from the future.
    Other(Version, Vec<u8>),
    V1(DiscoveryMessage<M>),
}

impl<D: Data, M: Multiaddress> TryInto<NetworkData<D, M>> for VersionedAuthentication<M> {
    type Error = Error;

    fn try_into(self) -> Result<NetworkData<D,M>, Self::Error> {
        use VersionedAuthentication::*;
        match self {
            V1(message) => Ok(NetworkData::Meta(message)),
            Other(v, _) => Err(Error::UnknownVersion(v)),
        }
    }
}

impl<M: Multiaddress> From<DiscoveryMessage<M>> for VersionedAuthentication<M> {
    fn from(message: DiscoveryMessage<M>) -> VersionedAuthentication<M> {
        VersionedAuthentication::V1(message)
    }
}

fn encode_with_version(version: Version, mut payload: Vec<u8>) -> Vec<u8> {
    let mut result = version.encode();
    // This will produce rubbish if we ever try encodings that have more than u32::MAX bytes.
    let num_bytes = payload.len() as ByteCount;
    result.append(&mut num_bytes.encode());
    result.append(&mut payload);
    result
}

impl<M: Multiaddress> Encode for VersionedAuthentication<M> {
    fn size_hint(&self) -> usize {
        use VersionedAuthentication::*;
        let version_size = size_of::<Version>();
        let byte_count_size = size_of::<ByteCount>();
        version_size
            + byte_count_size
            + match self {
                Other(_, payload) => payload.len(),
                V1(data) => data.size_hint(),
            }
    }

    fn encode(&self) -> Vec<u8> {
        use VersionedAuthentication::*;
        match self {
            Other(version, payload) => encode_with_version(*version, payload.clone()),
            V1(data) => encode_with_version(1, data.encode()),
        }
    }
}

impl<M: Multiaddress> Decode for VersionedAuthentication<M> {
    fn decode<I: CodecInput>(input: &mut I) -> Result<Self, CodecError> {
        use VersionedAuthentication::*;
        let version = Version::decode(input)?;
        let num_bytes = ByteCount::decode(input)?;
        match version {
            1 => Ok(V1(DiscoveryMessage::decode(input)?)),
            _ => {
                let mut payload = vec![
                    0;
                    num_bytes
                        .try_into()
                        .map_err(|_| "input too big to decode")?
                ];
                input.read(payload.as_mut_slice())?;
                Ok(Other(version, payload))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownVersion(Version),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            UnknownVersion(version) => {
                write!(
                    f,
                    "authentication data encoded with unknown version {}",
                    version
                )
            }
        }
    }
}
