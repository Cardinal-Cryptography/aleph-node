use codec::{Decode, Encode, Error, Input, Output};

use crate::{network::Data, SessionId};

#[derive(Encode, Eq, Decode, PartialEq, Debug, Copy, Clone)]
pub struct Version(pub u16);

pub trait Versioned {
    const VERSION: Version;
}

/// Wrapper for data send over network. We need it to ensure compatibility.
#[derive(Clone)]
pub struct VersionedNetworkDataWithSessionId<D: Data + Versioned> {
    pub data: D,
    pub session_id: SessionId,
}

impl<D: Data + Versioned> Decode for VersionedNetworkDataWithSessionId<D> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        // for now we dont use version for anything
        let _version = Version::decode(input)?;
        let data = D::decode(input)?;
        let session_id = SessionId::decode(input)?;

        Ok(VersionedNetworkDataWithSessionId { data, session_id })
    }
}

impl<D: Data + Versioned> Encode for VersionedNetworkDataWithSessionId<D> {
    fn size_hint(&self) -> usize {
        D::VERSION.size_hint() + self.data.size_hint() + self.session_id.size_hint()
    }

    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        D::VERSION.encode_to(dest);
        self.data.encode_to(dest);
        self.session_id.encode_to(dest);
    }
}
