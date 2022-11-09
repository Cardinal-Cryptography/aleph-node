use ink_prelude::{boxed::Box, vec, vec::Vec};
use ink_storage::traits::{PackedAllocate, PackedLayout, SpreadAllocate, SpreadLayout};
use scale::{Decode, Encode};
#[cfg(feature = "std")]
use scale_info::TypeInfo;

/// Abstraction for the elements kept in nodes (both in leaves and inner nodes).
#[cfg(feature = "std")]
pub trait TreeElement: Clone + PackedLayout + PackedAllocate + TypeInfo + 'static {}
#[cfg(feature = "std")]
impl<T: Clone + PackedLayout + PackedAllocate + TypeInfo + 'static> TreeElement for T {}

#[cfg(not(feature = "std"))]
pub trait TreeElement: Clone + PackedLayout + PackedAllocate {}
#[cfg(not(feature = "std"))]
impl<T: Clone + PackedLayout + PackedAllocate> TreeElement for T {}

/// Abstraction for a two-to-one hashing function.
///
/// This is used to compute hash in parent node from the children ones.
pub trait KinderBlender<H>: PackedLayout + Default {
    /// Compute hash from two hashes.
    fn blend_kinder(left: &H, right: &H) -> H;
}

/// Simplified binary tree that represents a Merkle tree over some set of hashes.
///
/// It has `LEAVES` leaves (and thus `2 * LEAVES - 1` nodes in general). `KB` is used for computing
/// values in the inner nodes. `TE` represents *both* leaves' values and inner nodes' values.
///
/// `LEAVES` must be power of `2`.
#[derive(Clone, Eq, PartialEq, Decode, Encode, PackedLayout, SpreadLayout, SpreadAllocate)]
#[cfg_attr(feature = "std", derive(TypeInfo, ink_storage::traits::StorageLayout))]
pub struct MerkleTree<TE: TreeElement, KB: KinderBlender<TE>, const LEAVES: usize> {
    /// Array of node values (root is at [1], children are in [2n] and [2n+1]).
    nodes: Vec<TE>,
    /// Marker of the first 'non-occupied' leaf.
    next_free_leaf: u32,
    #[codec(skip)]
    _phantom: Box<KB>,
}

/// Creates an 'empty' tree, i.e. every leaf contains `TE::default()`.
impl<TE: TreeElement + Default, KB: KinderBlender<TE>, const LEAVES: usize> Default
    for MerkleTree<TE, KB, LEAVES>
{
    fn default() -> Self {
        if !LEAVES.is_power_of_two() {
            panic!("Please have 2^n leaves")
        }

        let mut nodes = vec![TE::default(); 2 * LEAVES];
        for n in (LEAVES - 1)..1 {
            nodes[n] = KB::blend_kinder(&nodes[2 * n], &nodes[2 * n + 1]);
        }

        Self {
            nodes,
            next_free_leaf: LEAVES as u32,
            _phantom: Default::default(),
        }
    }
}

impl<TE: TreeElement, KB: KinderBlender<TE>, const LEAVES: usize> MerkleTree<TE, KB, LEAVES> {
    /// Get the value from the root node.
    pub fn root(&self) -> TE {
        self.nodes[1].clone()
    }

    /// Add `elem` to the first 'non-occupied' leaf. Returns `Err(())` iff there are no free leafs.
    pub fn add(&mut self, elem: TE) -> Result<(), ()> {
        if self.next_free_leaf as usize == 2 * LEAVES {
            return Err(());
        }

        self.nodes[self.next_free_leaf as usize] = elem;

        let mut parent = (self.next_free_leaf / 2) as usize;
        while parent > 0 {
            self.nodes[parent] =
                KB::blend_kinder(&self.nodes[2 * parent], &self.nodes[2 * parent + 1]);
            parent /= 2;
        }

        self.next_free_leaf += 1;

        Ok(())
    }
}
