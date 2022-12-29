use std::hash::{Hash, Hasher};

use sp_runtime::traits::{CheckedSub, Header as SubstrateHeader, One, UniqueSaturatedInto};

use crate::sync::{BlockIdentifier, Header};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockId<H: SubstrateHeader> {
    hash: H::Hash,
    number: H::Number,
}

impl<SH: SubstrateHeader> Hash for BlockId<SH> {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.hash.hash(state);
        self.number.hash(state);
    }
}

impl<H: SubstrateHeader> BlockIdentifier for BlockId<H> {
    fn number(&self) -> u32 {
        self.number.unique_saturated_into()
    }
}

impl<H: SubstrateHeader> Header for H {
    type Identifier = BlockId<H>;

    fn id(&self) -> Self::Identifier {
        BlockId {
            hash: self.hash(),
            number: *self.number(),
        }
    }

    fn parent_id(&self) -> Option<Self::Identifier> {
        let number = self.number().checked_sub(&One::one())?;
        Some(BlockId {
            hash: *self.parent_hash(),
            number,
        })
    }
}
