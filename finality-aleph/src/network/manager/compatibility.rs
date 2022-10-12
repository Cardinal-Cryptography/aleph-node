use std::{
    fmt::{Display, Error as FmtError, Formatter},
    mem::size_of,
};

use codec::{Decode, DecodeAll, Encode, Error as CodecError, Input as CodecInput};

use crate::network::{
    manager::{DiscoveryMessage, NetworkData},
    Data, Multiaddress,
};

type Version = u16;
type ByteCount = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionedNetworkData<D: Data, M: Multiaddress, LM: Multiaddress> {
    // Most likely from the future.
    Other(Version, Vec<u8>),
    Legacy(NetworkData<D, LM>),
    V1(DiscoveryMessage<M>),
}

fn encode_with_version(version: Version, mut payload: Vec<u8>) -> Vec<u8> {
    let mut result = version.encode();
    // This will produce rubbish if we ever try encodings that have more than u32::MAX bytes.
    let num_bytes = payload.len() as ByteCount;
    result.append(&mut num_bytes.encode());
    result.append(&mut payload);
    result
}

impl<D: Data, M: Multiaddress, LM: Multiaddress> Encode for VersionedNetworkData<D, M, LM> {
    fn size_hint(&self) -> usize {
        use VersionedNetworkData::*;
        let version_size = size_of::<Version>();
        let byte_count_size = size_of::<ByteCount>();
        version_size
            + byte_count_size
            + match self {
                Other(_, payload) => payload.len(),
                Legacy(data) => data.size_hint(),
                V1(data) => data.size_hint(),
            }
    }

    fn encode(&self) -> Vec<u8> {
        use VersionedNetworkData::*;
        match self {
            Other(version, payload) => encode_with_version(*version, payload.clone()),
            Legacy(data) => encode_with_version(0, data.encode()),
            V1(data) => encode_with_version(1, data.encode()),
        }
    }
}

impl<D: Data, M: Multiaddress, LM: Multiaddress> VersionedNetworkData<D, M, LM> {
    /// Encodes Network Data, to legacy encoding for the Legacy type. For other types
    /// it encodes to versioned encoding. This is reverse a funtion to `backwards_compatible_decode`.
    /// It is needed as encode needs to be a reverse function to decode, which is not possible without
    /// data having `ByteCount` and `Version` encoded. This is something pre-compatibility
    /// nodes will not understand.
    /// This should be removed, after rolling update with new network is completed
    pub fn backwards_compatible_encode(&self) -> Vec<u8> {
        use VersionedNetworkData::*;
        match self {
            Other(version, payload) => encode_with_version(*version, payload.clone()),
            Legacy(data) => data.encode(),
            V1(data) => encode_with_version(1, data.encode()),
        }
    }
}

impl<D: Data, M: Multiaddress, LM: Multiaddress> Decode for VersionedNetworkData<D, M, LM> {
    fn decode<I: CodecInput>(input: &mut I) -> Result<Self, CodecError> {
        use VersionedNetworkData::*;
        let version = Version::decode(input)?;
        let num_bytes = ByteCount::decode(input)?;
        match version {
            0 => Ok(Legacy(NetworkData::decode(input)?)),
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
    BadFormat,
    UnknownVersion(Version),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            BadFormat => write!(f, "malformed encoding"),
            UnknownVersion(version) => {
                write!(f, "network data encoded with unknown version {}", version)
            }
        }
    }
}

fn decode_pre_compatibility_network_data<D: Data, M: Multiaddress, LM: Multiaddress>(
    data_raw: Vec<u8>,
) -> Result<VersionedNetworkData<D, M, LM>, Error> {
    use Error::*;

    // We still have to be able to decode the pre-compatibility network data, so that
    // we can complete a rolling update. When it becomes obsolete we can remove it.
    let data_cloned = data_raw.clone();
    match NetworkData::decode_all(&mut data_cloned.as_slice()) {
        Ok(data) => Ok(VersionedNetworkData::Legacy(data.into())),
        Err(_) => Err(BadFormat),
    }
}

/// Decodes Network Data, even if it was produced by ancient code which does not conform to our
/// backwards compatibility style. This is reverse a funtion to `backwards_compatible_encode`.
/// This should be removed, after rolling update with new network is completed
pub fn backwards_compatible_decode<D: Data, M: Multiaddress, LM: Multiaddress>(
    data_raw: Vec<u8>,
) -> Result<VersionedNetworkData<D, M, LM>, Error> {
    use Error::*;
    let data_cloned = data_raw.clone();
    match VersionedNetworkData::<D, M, LM>::decode_all(&mut data_cloned.as_slice()) {
        Ok(data) => {
            use VersionedNetworkData::*;
            match data {
                Legacy(data) => Ok(Legacy(data)),
                V1(data) => Ok(V1(data)),
                Other(version, _) => {
                    // it is a coincidence that sometimes pre-compatibility legacy network data second word,
                    // which is in VersionedNetworkData byte_count_size, can be small enough
                    // so that network data is false positively recognized  as from the future
                    // therefore we should try to decode formats
                    decode_pre_compatibility_network_data(data_raw)
                        .map_err(|_| UnknownVersion(version))
                }
            }
        }
        Err(_) => decode_pre_compatibility_network_data(data_raw),
    }
}
