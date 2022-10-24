use codec::{Decode, Encode, Error, Input, Output};

use crate::{network::Data, SessionId};

#[derive(Encode, Eq, Decode, PartialEq, Debug, Copy, Clone)]
pub struct Version(pub u16);

pub trait Versioned {
    const VERSION: Version;
}

/// Wrapper for data send over network. We need it to ensure compatibility.
#[derive(Clone)]
pub struct NetworkDataInSession<D: Data> {
    pub data: D,
    pub session_id: SessionId,
}

impl<D: Data> Versioned for NetworkDataInSession<D> {
    const VERSION: Version = Version(0);
}

impl<D: Data> Decode for NetworkDataInSession<D> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let version = Version::decode(input)?;
        match version {
            Version(0) => {
                let data = D::decode(input)?;

                let session_id = SessionId::decode(input)?;
                Ok(NetworkDataInSession { data, session_id })
            }
            _ => Err("Invalid version while decoding NetworkDataInSession".into()),
        }
    }
}

impl<D: Data> Encode for NetworkDataInSession<D> {
    fn size_hint(&self) -> usize {
        Self::VERSION.size_hint() + self.data.size_hint() + self.session_id.size_hint()
    }

    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        Self::VERSION.encode_to(dest);
        self.data.encode_to(dest);
        self.session_id.encode_to(dest);
    }
}
