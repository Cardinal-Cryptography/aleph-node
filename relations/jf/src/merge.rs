use jf_relation::PlonkCircuit;

use crate::{
    note::{NoteGadget, NoteType, SourcedNote},
    shielder_types::{Note, Nullifier, TokenAmount, TokenId, Trapdoor},
    CircuitField, PlonkResult, PublicInput, Relation,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct MergeRelation {
    first_old_note: SourcedNote,
    second_old_note: SourcedNote,
    new_note: SourcedNote,
    first_merkle_path: MerkleProof,
}
