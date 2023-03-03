use liminal_ark_relation_macro::snark_relation;

/// It expresses the facts that:
///  - `first_old_note` is the result of hashing together the `token_id`,
///    `first_old_token_amount`, `first_old_trapdoor` and `first_old_nullifier`,
///  - `second_old_note` is the result of hashing together the `token_id`,
///    `second_old_token_amount`, `second_old_trapdoor` and `second_old_nullifier`,
///  - `new_note` is the result of hashing together the `token_id`, `new_token_amount`,
///    `new_trapdoor` and `new_nullifier`,
///  - `new_token_amount = token_amount + old_token_amount`
///  - `first_merkle_path` is a valid Merkle proof for `first_old_note` being present
///    at `first_leaf_index` in some Merkle tree with `merkle_root` hash in the root
///  - `second_merkle_path` is a valid Merkle proof for `second_old_note` being present
///    at `second_leaf_index` in some Merkle tree with `merkle_root` hash in the root
/// Additionally, the relation has one constant input, `max_path_len` which specifies upper bound
/// for the length of the merkle path (which is ~the height of the tree, ±1).
#[snark_relation]
mod relation {
    use core::ops::Add;

    use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget, fields::fp::FpVar};
    use ark_relations::ns;

    use crate::shielder::{
        check_merkle_proof,
        circuit_utils::PathShapeVar,
        convert_hash, convert_vec,
        note::check_note,
        types::{
            BackendLeafIndex, BackendMerklePath, BackendMerkleRoot, BackendNote, BackendNullifier,
            BackendTokenAmount, BackendTokenId, BackendTrapdoor, FrontendLeafIndex,
            FrontendMerklePath, FrontendMerkleRoot, FrontendNote, FrontendNullifier,
            FrontendTokenAmount, FrontendTokenId, FrontendTrapdoor,
        },
    };

    #[relation_object_definition]
    struct MergeRelation {
        #[constant]
        pub max_path_len: u8,

        // Public inputs
        #[public_input(frontend_type = "FrontendTokenId")]
        pub token_id: BackendTokenId,
        #[public_input(frontend_type = "FrontendNullifier")]
        pub first_old_nullifier: BackendNullifier,
        #[public_input(frontend_type = "FrontendNullifier")]
        pub second_old_nullifier: BackendNullifier,
        #[public_input(frontend_type = "FrontendNote", parse_with = "convert_hash")]
        pub new_note: BackendNote,
        #[public_input(frontend_type = "FrontendMerkleRoot", parse_with = "convert_hash")]
        pub merkle_root: BackendMerkleRoot,

        // Private inputs.
        #[private_input(frontend_type = "FrontendTrapdoor")]
        pub first_old_trapdoor: BackendTrapdoor,
        #[private_input(frontend_type = "FrontendTrapdoor")]
        pub second_old_trapdoor: BackendTrapdoor,
        #[private_input(frontend_type = "FrontendTrapdoor")]
        pub new_trapdoor: BackendTrapdoor,
        #[private_input(frontend_type = "FrontendNullifier")]
        pub new_nullifier: BackendNullifier,
        #[private_input(frontend_type = "FrontendMerklePath", parse_with = "convert_vec")]
        pub first_merkle_path: BackendMerklePath,
        #[private_input(frontend_type = "FrontendMerklePath", parse_with = "convert_vec")]
        pub second_merkle_path: BackendMerklePath,
        #[private_input(frontend_type = "FrontendLeafIndex")]
        pub first_leaf_index: BackendLeafIndex,
        #[private_input(frontend_type = "FrontendLeafIndex")]
        pub second_leaf_index: BackendLeafIndex,
        #[private_input(frontend_type = "FrontendNote", parse_with = "convert_hash")]
        pub first_old_note: BackendNote,
        #[private_input(frontend_type = "FrontendNote", parse_with = "convert_hash")]
        pub second_old_note: BackendNote,
        #[private_input(frontend_type = "FrontendTokenAmount")]
        pub first_old_token_amount: BackendTokenAmount,
        #[private_input(frontend_type = "FrontendTokenAmount")]
        pub second_old_token_amount: BackendTokenAmount,
        #[private_input(frontend_type = "FrontendTokenAmount")]
        pub new_token_amount: BackendTokenAmount,
    }

    #[circuit_definition]
    fn generate_constraints() {
        let token_id = FpVar::new_input(ns!(cs, "token id"), || self.token_id())?;
        //------------------------------
        // Check first old note arguments.
        //------------------------------
        let first_old_token_amount = FpVar::new_witness(ns!(cs, "first old token amount"), || {
            self.first_old_token_amount()
        })?;
        let first_old_trapdoor =
            FpVar::new_witness(ns!(cs, "first old trapdoor"), || self.first_old_trapdoor())?;
        let first_old_nullifier = FpVar::new_input(ns!(cs, "first old nullifier"), || {
            self.first_old_nullifier()
        })?;
        let first_old_note =
            FpVar::new_witness(ns!(cs, "first old note"), || self.first_old_note())?;

        check_note(
            &token_id,
            &first_old_token_amount,
            &first_old_trapdoor,
            &first_old_nullifier,
            &first_old_note,
        )?;

        //------------------------------
        // Check second old note arguments.
        //------------------------------
        let second_old_token_amount =
            FpVar::new_witness(ns!(cs, "second old token amount"), || {
                self.second_old_token_amount()
            })?;
        let second_old_trapdoor = FpVar::new_witness(ns!(cs, "second old trapdoor"), || {
            self.second_old_trapdoor()
        })?;
        let second_old_nullifier = FpVar::new_input(ns!(cs, "second old nullifier"), || {
            self.second_old_nullifier()
        })?;
        let second_old_note =
            FpVar::new_witness(ns!(cs, "second old note"), || self.second_old_note())?;

        check_note(
            &token_id,
            &second_old_token_amount,
            &second_old_trapdoor,
            &second_old_nullifier,
            &second_old_note,
        )?;

        //------------------------------
        // Check new note arguments.
        //------------------------------
        let new_token_amount =
            FpVar::new_witness(ns!(cs, "new token amount"), || self.new_token_amount())?;
        let new_trapdoor = FpVar::new_witness(ns!(cs, "new trapdoor"), || self.new_trapdoor())?;
        let new_nullifier = FpVar::new_witness(ns!(cs, "new nullifier"), || self.new_nullifier())?;
        let new_note = FpVar::new_input(ns!(cs, "new note"), || self.new_note())?;

        check_note(
            &token_id,
            &new_token_amount,
            &new_trapdoor,
            &new_nullifier,
            &new_note,
        )?;

        //----------------------------------
        // Check token value soundness.
        //----------------------------------
        // some range checks for overflows?
        let token_sum = first_old_token_amount.add(second_old_token_amount);
        token_sum.enforce_equal(&new_token_amount)?;

        //------------------------
        // Check first merkle proof.
        //------------------------
        let merkle_root = FpVar::new_input(ns!(cs, "merkle root"), || self.merkle_root())?;
        let first_path_shape = PathShapeVar::new_witness(ns!(cs, "first path shape"), || {
            Ok((*self.max_path_len(), self.first_leaf_index().cloned()))
        })?;

        check_merkle_proof(
            merkle_root.clone(),
            first_path_shape,
            first_old_note,
            self.first_merkle_path().cloned().unwrap_or_default(),
            *self.max_path_len(),
            cs.clone(),
        )?;

        //------------------------
        // Check second merkle proof.
        //------------------------
        let second_path_shape = PathShapeVar::new_witness(ns!(cs, "second path shape"), || {
            Ok((*self.max_path_len(), self.second_leaf_index().cloned()))
        })?;

        check_merkle_proof(
            merkle_root,
            second_path_shape,
            second_old_note,
            self.second_merkle_path().cloned().unwrap_or_default(),
            *self.max_path_len(),
            cs,
        )
    }
}

#[cfg(test)]
mod tests {
    use ark_bls12_381::Bls12_381;
    use ark_groth16::Groth16;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
    use ark_snark::SNARK;

    use super::*;
    use crate::{
        shielder::note::{compute_note, compute_parent_hash},
        FrontendNote,
    };

    const MAX_PATH_LEN: u8 = 4;

    fn get_circuit_with_full_input() -> MergeRelationWithFullInput {
        let token_id: FrontendTokenId = 1;

        let first_old_trapdoor: FrontendTrapdoor = 17;
        let first_old_nullifier: FrontendNullifier = 19;
        let first_old_token_amount: FrontendTokenAmount = 3;

        let second_old_trapdoor: FrontendTrapdoor = 23;
        let second_old_nullifier: FrontendNullifier = 29;
        let second_old_token_amount: FrontendTokenAmount = 7;

        let new_trapdoor: FrontendTrapdoor = 27;
        let new_nullifier: FrontendNullifier = 87;
        let new_token_amount: FrontendTokenAmount = 10;

        let first_old_note = compute_note(
            token_id,
            first_old_token_amount,
            first_old_trapdoor,
            first_old_nullifier,
        );
        let second_old_note = compute_note(
            token_id,
            second_old_token_amount,
            second_old_trapdoor,
            second_old_nullifier,
        );
        let new_note = compute_note(token_id, new_token_amount, new_trapdoor, new_nullifier);

        //                                          merkle root
        //                placeholder                                        x
        //        1                       x                     x                       x
        //   2         3              x        x            x       x              x       x
        // 4  *5*  ^6^   7          x   x    x   x        x   x   x   x          x   x   x   x
        //
        // *first_old_note* | ^second_old_note^

        let zero_note = FrontendNote::default(); // x

        // First Merkle path setup.
        let first_leaf_index = 5;
        let first_sibling_note = compute_note(0, 1, 2, 3); // 4
        let first_parent_note = compute_parent_hash(first_sibling_note, first_old_note); // 2

        // Second Merkle path setup.
        let second_leaf_index = 6;
        let second_sibling_note = compute_note(0, 1, 3, 4); // 7
        let second_parent_note = compute_parent_hash(second_old_note, second_sibling_note); // 3

        // Merkle paths.
        let first_merkle_path = vec![first_sibling_note, second_parent_note];
        let second_merkle_path = vec![second_sibling_note, first_parent_note];

        // Common roots.
        let grandpa_root = compute_parent_hash(first_parent_note, second_parent_note); // 1
        let placeholder = compute_parent_hash(grandpa_root, zero_note);
        let merkle_root = compute_parent_hash(placeholder, zero_note);

        MergeRelationWithFullInput::new(
            MAX_PATH_LEN,
            token_id,
            first_old_nullifier,
            second_old_nullifier,
            new_note,
            merkle_root,
            first_old_trapdoor,
            second_old_trapdoor,
            new_trapdoor,
            new_nullifier,
            first_merkle_path,
            second_merkle_path,
            first_leaf_index,
            second_leaf_index,
            first_old_note,
            second_old_note,
            first_old_token_amount,
            second_old_token_amount,
            new_token_amount,
        )
    }

    #[test]
    fn merge_constraints_correctness() {
        let circuit = get_circuit_with_full_input();

        let cs = ConstraintSystem::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        let is_satisfied = cs.is_satisfied().unwrap();
        println!("Dat: {:?}", cs.num_constraints());
        if !is_satisfied {
            println!("{:?}", cs.which_is_unsatisfied());
        }

        assert!(is_satisfied);
    }

    #[test]
    fn merge_proving_procedure() {
        let circuit_withouth_input = MergeRelationWithoutInput::new(MAX_PATH_LEN);

        let mut rng = ark_std::test_rng();
        let (pk, vk) =
            Groth16::<Bls12_381>::circuit_specific_setup(circuit_withouth_input, &mut rng).unwrap();

        let circuit = get_circuit_with_full_input();
        let proof = Groth16::prove(&pk, circuit, &mut rng).unwrap();

        let circuit: MergeRelationWithPublicInput = get_circuit_with_full_input().into();
        let input = circuit.serialize_public_input();
        let valid_proof = Groth16::verify(&vk, &input, &proof).unwrap();
        assert!(valid_proof);
    }
}
