use jf_primitives::merkle_tree::{
    prelude::RescueSparseMerkleTree, MerkleTreeScheme, UniversalMerkleTreeScheme,
};
use jf_relation::PlonkCircuit;
use num_bigint::BigUint;

use crate::{
    note::{NoteGadget, NoteType, SourcedNote},
    shielder_types::{
        convert_account, convert_array, Account, LeafIndex, MerkleRoot, Note, Nullifier,
        TokenAmount, TokenId, Trapdoor,
    },
    CircuitField, MerkleProof, PlonkResult, PublicInput, Relation, MERKLE_TREE_HEIGHT,
};

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MergeRelation {
    //
    first_old_note: SourcedNote,
    second_old_note: SourcedNote,
    new_note: SourcedNote,
    //
    merkle_root: MerkleRoot,
    //
    first_merkle_path: MerkleProof,
    first_leaf_index: LeafIndex,
    //
    second_merkle_path: MerkleProof,
    second_leaf_index: LeafIndex,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct MergePublicInput {
    pub token_id: TokenId,
    pub first_old_nullifier: Nullifier,
    pub second_old_nullifier: Nullifier,
    pub new_note: Note,
    pub merkle_root: MerkleRoot,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MergePrivateInput {
    pub first_old_trapdoor: Trapdoor,
    pub second_old_trapdoor: Trapdoor,
    pub new_trapdoor: Trapdoor,
    pub new_nullifier: Nullifier,
    pub first_merkle_path: MerkleProof,
    pub second_merkle_path: MerkleProof,
    pub first_leaf_index: LeafIndex,
    pub second_leaf_index: LeafIndex,
    pub first_old_note: Note,
    pub second_old_note: Note,
    pub first_old_token_amount: TokenAmount,
    pub second_old_token_amount: TokenAmount,
    pub new_token_amount: TokenAmount,
}

impl Default for MergePrivateInput {
    fn default() -> Self {
        let index = BigUint::from(0u64);
        let value = CircuitField::from(0u64);

        let merkle_tree =
            RescueSparseMerkleTree::from_kv_set(MERKLE_TREE_HEIGHT, &[(index.clone(), value)])
                .unwrap();

        let (_, merkle_proof) = merkle_tree.lookup(&index).expect_ok().unwrap();

        Self {
            first_old_trapdoor: Default::default(),
            second_old_trapdoor: Default::default(),
            new_trapdoor: Default::default(),
            new_nullifier: Default::default(),
            first_merkle_path: merkle_proof.clone(),
            second_merkle_path: merkle_proof,
            first_leaf_index: Default::default(),
            second_leaf_index: Default::default(),
            first_old_note: Default::default(),
            second_old_note: Default::default(),
            first_old_token_amount: Default::default(),
            second_old_token_amount: Default::default(),
            new_token_amount: Default::default(),
        }
    }
}

impl MergeRelation {
    pub fn new(public: MergePublicInput, private: MergePrivateInput) -> Self {
        let first_old_note = SourcedNote {
            note: private.first_old_note,
            token_id: public.token_id,
            token_amount: private.first_old_token_amount,
            trapdoor: private.first_old_trapdoor,
            nullifier: public.first_old_nullifier,
            note_type: NoteType::Deposit,
        };

        let second_old_note = SourcedNote {
            note: private.second_old_note,
            token_id: public.token_id,
            token_amount: private.second_old_token_amount,
            trapdoor: private.second_old_trapdoor,
            nullifier: public.second_old_nullifier,
            note_type: NoteType::Deposit,
        };

        let new_note = SourcedNote {
            note: public.new_note,
            token_id: public.token_id,
            token_amount: private.new_token_amount,
            trapdoor: private.new_trapdoor,
            nullifier: private.new_nullifier,
            note_type: NoteType::Deposit,
        };

        Self {
            first_old_note,
            second_old_note,
            new_note,
            merkle_root: todo!(),
            first_merkle_path: todo!(),
            first_leaf_index: todo!(),
            second_merkle_path: todo!(),
            second_leaf_index: todo!(),
        }
    }
}

impl Default for MergeRelation {
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}
