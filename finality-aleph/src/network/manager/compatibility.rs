use codec::{Decode, Encode};

use crate::{
    compatibility::{MessageVersioning, UnrecognizedVersionError, Version},
    network::{manager::NetworkData, Data, Multiaddress},
};

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub enum VersionedNetworkData<D: Data, M: Multiaddress> {
    Other(Version, Vec<u8>),
    V1(NetworkData<D, M>),
}

impl<D: Data, M: Multiaddress> MessageVersioning for NetworkData<D, M> {
    type Versioned = VersionedNetworkData<D, M>;

    fn into_versioned(self) -> Self::Versioned {
        VersionedNetworkData::V1(self)
    }

    fn from_versioned(versioned: Self::Versioned) -> Result<Self, UnrecognizedVersionError> {
        match versioned {
            VersionedNetworkData::V1(data) => Ok(data),
            VersionedNetworkData::Other(version, value) => {
                Err(UnrecognizedVersionError { version, value })
            }
        }
    }
}
