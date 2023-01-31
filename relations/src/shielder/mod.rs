//! This module contains two relations that are the core of the Shielder application: `deposit` and
//! `withdraw`. It also exposes some functions and types that might be useful for input generation.
//!
//! Currently, instead of using some real hash function, we chose to incorporate a simple tangling
//! algorithm. Essentially, it is a procedure that just mangles a byte sequence.

mod deposit;
mod deposit_and_merge;
mod note;
mod tangle;
pub mod types;
mod withdraw;

use ark_ff::{BigInteger256, PrimeField, Zero};
use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget};
use ark_relations::{
    ns,
    r1cs::{ConstraintSystemRef, SynthesisError, SynthesisError::UnconstrainedVariable},
};
use ark_std::vec::Vec;
pub use deposit::{
    DepositRelationWithFullInput, DepositRelationWithPublicInput, DepositRelationWithoutInput,
};
pub use deposit_and_merge::{
    DepositAndMergeRelationWithFullInput, DepositAndMergeRelationWithPublicInput,
    DepositAndMergeRelationWithoutInput,
};
pub use note::{bytes_from_note, compute_note, compute_parent_hash, note_from_bytes};
use tangle::tangle_in_field;
use types::{BackendMerklePath, BackendMerkleRoot};
pub use types::{
    FrontendMerklePath as MerklePath, FrontendMerkleRoot as MerkleRoot, FrontendNote as Note,
    FrontendNullifier as Nullifier, FrontendTokenAmount as TokenAmount, FrontendTokenId as TokenId,
    FrontendTrapdoor as Trapdoor,
};
pub use withdraw::{
    WithdrawRelationWithFullInput, WithdrawRelationWithPublicInput, WithdrawRelationWithoutInput,
};

use crate::environment::{CircuitField, FpVar};

fn convert_hash(front: [u64; 4]) -> CircuitField {
    CircuitField::new(BigInteger256::new(front))
}

fn convert_vec(front: Vec<[u64; 4]>) -> Vec<CircuitField> {
    front.into_iter().map(convert_hash).collect()
}

fn convert_account(front: [u8; 32]) -> CircuitField {
    CircuitField::from_le_bytes_mod_order(&front)
}

fn check_merkle_proof(
    merkle_root: Result<&BackendMerkleRoot, SynthesisError>,
    leaf_index: Result<&u64, SynthesisError>,
    leaf: FpVar,
    path: BackendMerklePath,
    max_path_len: u8,
    cs: ConstraintSystemRef<CircuitField>,
) -> Result<(), SynthesisError> {
    if path.len() > max_path_len as usize {
        return Err(UnconstrainedVariable);
    }

    let merkle_root = FpVar::new_input(ns!(cs, "merkle root"), || merkle_root)?;
    let _ = FpVar::new_witness(ns!(cs, "leaf index"), || {
        leaf_index.map(|li| CircuitField::from(*li))
    })?;
    let mut leaf_index = leaf_index.cloned().unwrap_or_default();

    let mut current_note = leaf;
    let zero_note = CircuitField::zero();

    for i in 0..max_path_len {
        let sibling = FpVar::new_witness(ns!(cs, "merkle path node"), || {
            Ok(path.get(i as usize).unwrap_or(&zero_note))
        })?;
        let next_level = if leaf_index & 1 == 0 {
            [current_note.clone(), sibling]
        } else {
            [sibling, current_note.clone()]
        };

        current_note = tangle_in_field(&next_level)?;
        leaf_index /= 2;
    }

    merkle_root.enforce_equal(&current_note)
}
