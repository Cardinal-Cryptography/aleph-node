use jf_relation::PlonkCircuit;

use crate::{
    note::{NoteGadget, NoteType, SourcedNote},
    shielder_types::{
        convert_account, convert_array, Account, LeafIndex, MerkleRoot, Note, Nullifier,
        TokenAmount, TokenId, Trapdoor,
    },
    CircuitField, MerkleProof, PlonkResult, PublicInput, Relation,
};

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MergeRelation {
    first_old_note: SourcedNote,
    second_old_note: SourcedNote,
    new_note: SourcedNote,
    first_merkle_path: MerkleProof,
}
