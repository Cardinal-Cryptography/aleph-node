mod environment;
mod linear;
mod merkle_tree;
mod serialization;
mod shielder;
// mod types;
mod utils;
mod xor;

use ark_ff::{One, PrimeField, Zero};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef};
use ark_serialize::CanonicalSerialize;
pub use linear::LinearEquationRelation;
pub use merkle_tree::MerkleTreeRelation;
pub use serialization::serialize;
pub use shielder::{note_from_bytes, types::*, DepositRelation, WithdrawRelation};
pub use utils::*;
// use types::CircuitField;
pub use xor::XorRelation;

// All implemented relations.
//
// They should have corresponding definition in submodule.
// #[derive(Clone)]
// pub enum Relation {
//     Xor(XorRelation),
//     LinearEquation(LinearEqRelation),
//     MerkleTree(MerkleTreeRelation),
//     Deposit(DepositRelation),
//     Withdraw(WithdrawRelation),
// }

// impl Relation {
//     /// Relation identifier.
//     pub fn id(&self) -> String {
//         match &self {
//             Relation::Xor(_) => String::from("xor"),
//             Relation::LinearEquation(_) => String::from("linear_equation"),
//             Relation::MerkleTree(_) => String::from("merkle_tree"),
//             Relation::Deposit(_) => String::from("deposit"),
//             Relation::Withdraw(_) => String::from("withdraw"),
//         }
//     }
// }

// impl ConstraintSynthesizer<CircuitField> for Relation {
//     fn generate_constraints(
//         self,
//         cs: ConstraintSystemRef<CircuitField>,
//     ) -> ark_relations::r1cs::Result<()> {
//         match self {
//             Relation::Xor(relation @ XorRelation { .. }) => relation.generate_constraints(cs),

//             Relation::LinearEquation(relation @ LinearEqRelation { .. }) => {
//                 relation.generate_constraints(cs)
//             }

//             Relation::MerkleTree(relation @ MerkleTreeRelation { .. }) => {
//                 relation.generate_constraints(cs)
//             }

//             Relation::Deposit(relation @ DepositRelation { .. }) => {
//                 relation.generate_constraints(cs)
//             }

//             Relation::Withdraw(relation @ WithdrawRelation { .. }) => {
//                 relation.generate_constraints(cs)
//             }
//         }
//     }
// }

pub trait GetPublicInput<CircuitField: PrimeField + CanonicalSerialize> {
    fn public_input(&self) -> Vec<CircuitField> {
        vec![]
    }
}

// impl GetPublicInput<CircuitField> for Relation {
//     fn public_input(&self) -> Vec<CircuitField> {
//         match self {
//             Relation::Xor(relation @ XorRelation { .. }) => relation.public_input(),
//             Relation::LinearEquation(relation @ LinearEqRelation { .. }) => relation.public_input(),
//             Relation::MerkleTree(relation @ MerkleTreeRelation { .. }) => relation.public_input(),
//             Relation::Deposit(relation @ DepositRelation { .. }) => relation.public_input(),
//             Relation::Withdraw(relation @ WithdrawRelation { .. }) => relation.public_input(),
//         }
//     }
// }
