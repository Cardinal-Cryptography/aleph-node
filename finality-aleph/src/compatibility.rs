use codec::{Decode, Encode, Error, Input, Output};

use crate::{network::Data, SessionId};

#[derive(Encode, Eq, Decode, PartialEq, Debug, Copy, Clone)]
pub struct Version(pub u16);

pub trait Versioned {
    const VERSION: Version;
}

/// Wrapper for data send over network. We need it to ensure compatibility.
/// The order of the data and session_id is fixed in encode and the decode expects it to be data, session_id.
#[derive(Clone)]
pub struct NetworkDataInSession<D: Data> {
    pub data: D,
    pub session_id: SessionId,
}

impl<D: Data> Decode for NetworkDataInSession<D> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let data = D::decode(input)?;
        let session_id = SessionId::decode(input)?;

        Ok(Self { data, session_id })
    }
}

impl<D: Data> Encode for NetworkDataInSession<D> {
    fn size_hint(&self) -> usize {
        self.data.size_hint() + self.session_id.size_hint()
    }

    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        self.data.encode_to(dest);
        self.session_id.encode_to(dest);
    }
}
