pub struct NodeIndex(pub(crate) u32);

impl From<u32> for NodeIndex {
    fn from(idx: u32) -> Self {
        NodeIndex(idx)
    }
}
