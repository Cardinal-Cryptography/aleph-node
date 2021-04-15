use codec::{Decode, Encode};

#[derive(Clone, Encode, Decode, PartialEq, Eq, Debug)]
pub struct AlephJustification {
    pub(crate) data: Vec<u8>,
}

impl AlephJustification {
    pub fn trivial_proof() -> Self {
        Self {
            data: vec![0, 1, 2, 3],
        }
    }
}
