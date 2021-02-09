use codec::{Decode, Encode};
use std::fmt::{Display, Result as FmtResult, Formatter};

#[derive(Debug, Clone, Copy, Encode, Decode)]
pub struct NodeIndex(pub(crate) u32);

impl Display for NodeIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for NodeIndex {
    fn from(idx: u32) -> Self {
        NodeIndex(idx)
    }
}
