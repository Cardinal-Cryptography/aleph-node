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
