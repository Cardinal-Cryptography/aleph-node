use jf_primitives::merkle_tree::{
    prelude::RescueSparseMerkleTree, MerkleTreeScheme, UniversalMerkleTreeScheme,
};
use jf_relation::{Circuit, PlonkCircuit};
use num_bigint::BigUint;

use crate::{
    check_merkle_proof,
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
            note_type: NoteType::Spend,
        };

        let second_old_note = SourcedNote {
            note: private.second_old_note,
            token_id: public.token_id,
            token_amount: private.second_old_token_amount,
            trapdoor: private.second_old_trapdoor,
            nullifier: public.second_old_nullifier,
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
            first_old_note,
            second_old_note,
            new_note,
            merkle_root: public.merkle_root,
            first_merkle_path: private.first_merkle_path,
            first_leaf_index: private.first_leaf_index,
            second_merkle_path: private.second_merkle_path,
            second_leaf_index: private.second_leaf_index,
        }
    }
}

impl Default for MergeRelation {
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

impl PublicInput for MergeRelation {
    fn public_input(&self) -> Vec<CircuitField> {
        let mut public_input = Vec::new();
        public_input.push(convert_array(self.merkle_root));
        public_input.extend(self.first_old_note.public_input());
        public_input.extend(self.second_old_note.public_input());
        public_input.extend(self.new_note.public_input());
        public_input
    }
}

impl Relation for MergeRelation {
    fn generate_subcircuit(&self, circuit: &mut PlonkCircuit<CircuitField>) -> PlonkResult<()> {
        //------------------------------
        // first_old_note = H(token_id, first_old_token_amount, first_old_trapdoor, first_old_nullifier)
        //------------------------------
        let first_old_note_var = circuit.create_note_variable(&self.first_old_note)?;
        let first_old_note_token_amount_var = first_old_note_var.token_amount_var;
        circuit.enforce_note_preimage(first_old_note_var)?;

        //------------------------------
        // second_old_note = H(token_id, first_old_token_amount, first_old_trapdoor, first_old_nullifier)
        //------------------------------
        let second_old_note_var = circuit.create_note_variable(&self.second_old_note)?;
        let second_old_note_token_amount_var = second_old_note_var.token_amount_var;
        circuit.enforce_note_preimage(second_old_note_var)?;

        //------------------------------
        // new_note = H(token_id, new_token_amount, new_trapdoor, new_nullifier)
        //------------------------------
        let new_note_var = circuit.create_note_variable(&self.new_note)?;
        let new_note_token_amount_var = new_note_var.token_amount_var;
        circuit.enforce_note_preimage(new_note_var)?;

        //------------------------------
        //  first_merkle_path is a valid Merkle proof for first_old_note being present
        //  at first_leaf_index in a Merkle tree with merkle_root hash in the root
        //------------------------------
        check_merkle_proof(
            circuit,
            self.first_leaf_index,
            self.merkle_root,
            &self.first_merkle_path,
        )?;

        //------------------------------
        //  second_merkle_path is a valid Merkle proof for second_old_note being present
        //  at first_leaf_index in a Merkle tree with merkle_root hash in the root
        //------------------------------
        check_merkle_proof(
            circuit,
            self.second_leaf_index,
            self.merkle_root,
            &self.second_merkle_path,
        )?;

        //------------------------------
        //  new_token_amount = token_amount + old_token_amount
        //------------------------------

        let old_notes_token_amount_sum_var = circuit.add(
            first_old_note_token_amount_var,
            second_old_note_token_amount_var,
        )?;
        circuit.enforce_equal(old_notes_token_amount_sum_var, new_note_token_amount_var)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shielder_types::compute_note;

    fn merge_relation() -> MergeRelation {
        let token_id = 1;

        let first_old_token_amount = 7;
        let first_old_trapdoor = [1; 4];
        let first_old_nullifier = [2; 4];

        let first_old_note = compute_note(
            token_id,
            first_old_token_amount,
            first_old_trapdoor,
            first_old_nullifier,
        );

        todo!()
    }

    #[test]
    fn merge_constraints_correctness() {
        let relation = merge_relation();
        let circuit = MergeRelation::generate_circuit(&relation).unwrap();

        circuit
            .check_circuit_satisfiability(&relation.public_input())
            .unwrap();
    }
}
