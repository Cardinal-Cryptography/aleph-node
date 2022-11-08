use ink_prelude::{boxed::Box, vec::Vec};
use ink_storage::traits::{PackedAllocate, PackedLayout, SpreadAllocate, SpreadLayout};
use scale::{Decode, Encode};
#[cfg(feature = "std")]
use scale_info::TypeInfo;

#[cfg(feature = "std")]
pub trait TreeElement: Clone + PackedLayout + PackedAllocate + TypeInfo + 'static {}
#[cfg(feature = "std")]
impl<T: Clone + PackedLayout + PackedAllocate + TypeInfo + 'static> TreeElement for T {}

#[cfg(not(feature = "std"))]
pub trait TreeElement: Clone + PackedLayout + PackedAllocate {}
#[cfg(not(feature = "std"))]
impl<T: Clone + PackedLayout + PackedAllocate> TreeElement for T {}

pub trait KinderBlender<T>: PackedLayout + Default {
    fn blend_kinder(left: &T, right: &T) -> T;
}

#[derive(Clone, Eq, PartialEq, Decode, Encode, PackedLayout, SpreadLayout, SpreadAllocate)]
#[cfg_attr(feature = "std", derive(TypeInfo, ink_storage::traits::StorageLayout))]
pub struct MerkleTree<TE: TreeElement, KB: KinderBlender<TE>, const LEAVES: usize> {
    nodes: Vec<TE>,
    next_free_leaf: u32,
    #[codec(skip)]
    _phantom: Box<KB>,
}

impl<TE: TreeElement, KB: KinderBlender<TE>, const LEAVES: usize> Default
    for MerkleTree<TE, KB, LEAVES>
{
    fn default() -> Self {
        if !LEAVES.is_power_of_two() {
            panic!("Please have 2^n leaves")
        }

        let mut nodes = Vec::with_capacity(2 * LEAVES);
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
    pub fn root(&self) -> TE {
        self.nodes[1].clone()
    }

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

        Ok(())
    }
}
