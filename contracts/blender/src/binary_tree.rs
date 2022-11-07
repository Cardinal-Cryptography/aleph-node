use ink_storage::traits::{PackedAllocate, PackedLayout, SpreadAllocate, SpreadLayout};
use scale::{Decode, Encode};
use sp_std::vec::Vec;

#[cfg(feature = "std")]
pub trait TreeElement: PackedLayout + PackedAllocate + scale_info::TypeInfo + 'static {}
#[cfg(feature = "std")]
impl<T: PackedLayout + PackedAllocate + scale_info::TypeInfo + 'static> TreeElement for T {}

#[cfg(not(feature = "std"))]
pub trait TreeElement: PackedLayout + PackedAllocate {}
#[cfg(not(feature = "std"))]
impl<T: PackedLayout + PackedAllocate> TreeElement for T {}

#[derive(Clone, Eq, PartialEq, Decode, Encode, PackedLayout, SpreadLayout, SpreadAllocate)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
)]
pub struct BinaryTree<T: TreeElement, const LEAVES: usize> {
    nodes: Vec<T>,
}

impl<T: TreeElement, const LEAVES: usize> Default for BinaryTree<T, LEAVES> {
    fn default() -> Self {
        Self {
            nodes: Vec::with_capacity(2 * LEAVES),
        }
    }
}
