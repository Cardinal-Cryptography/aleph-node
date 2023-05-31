use jf_primitives::merkle_tree::{
    prelude::RescueSparseMerkleTree, MerkleTreeScheme, UniversalMerkleTreeScheme,
};
use jf_relation::{Circuit, PlonkCircuit};
use num_bigint::BigUint;

use crate::{
    check_merkle_proof,
    note::{NoteGadget, NoteType, SourcedNote},
    shielder_types::{
        convert_array, LeafIndex, MerkleRoot, Note, Nullifier, TokenAmount, TokenId, Trapdoor,
    },
    CircuitField, MerkleProof, PlonkResult, PublicInput, Relation, MERKLE_TREE_HEIGHT,
};

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct DepositAndMergeRelation {
    old_note: SourcedNote,
    new_note: SourcedNote,
    merkle_path: MerkleProof,
    leaf_index: LeafIndex,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositAndMergePublicInput {
    pub merkle_root: MerkleRoot,
    pub new_note: Note,
    pub old_nullifier: Nullifier,
    pub token_amount: TokenAmount,
    pub token_id: TokenId,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct DepositAndMergePrivateInput {
    pub old_trapdoor: Trapdoor,
    pub new_trapdoor: Trapdoor,
    pub new_nullifier: Nullifier,
    pub merkle_path: MerkleProof,
    pub leaf_index: LeafIndex,
    pub old_note: Note,
    pub old_token_amount: TokenAmount,
    pub new_token_amount: TokenAmount,
}

impl Default for DepositAndMergePrivateInput {
    fn default() -> Self {
        let index = BigUint::from(0u64);
        let value = CircuitField::from(0u64);

        let merkle_tree =
            RescueSparseMerkleTree::from_kv_set(MERKLE_TREE_HEIGHT, &[(index.clone(), value)])
                .unwrap();

        let (_, merkle_proof) = merkle_tree.lookup(&index).expect_ok().unwrap();

        Self {
            old_trapdoor: Default::default(),
            new_trapdoor: Default::default(),
            new_nullifier: Default::default(),
            merkle_path: merkle_proof,
            leaf_index: Default::default(),
            old_note: Default::default(),
            old_token_amount: Default::default(),
            new_token_amount: Default::default(),
        }
    }
}

impl DepositAndMergeRelation {
    pub fn new(public: DepositAndMergePublicInput, private: DepositAndMergePrivateInput) -> Self {
        let old_note = SourcedNote {
            note: private.old_note,
            token_id: public.token_id,
            token_amount: private.old_token_amount,
            trapdoor: private.old_trapdoor,
            nullifier: public.old_nullifier,
            note_type: NoteType::Spend,
        };

        let new_note = SourcedNote {
            note: public.new_note,
            token_id: public.token_id,
            token_amount: private.new_token_amount,
            trapdoor: private.new_trapdoor,
            nullifier: private.new_nullifier,
            note_type: NoteType::Redeposit,
        };

        Self {
            old_note,
            new_note,
            merkle_path: private.merkle_path,
            leaf_index: private.leaf_index,
        }
    }
}
