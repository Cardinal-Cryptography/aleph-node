use ark_r1cs_std::prelude::{AllocVar, EqGadget, UInt8};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef};
pub type ConstraintF = ark_bls12_381::Fr;

/// Relation with:
///  - 1 public input    (a | `public_xoree`)
///  - 1 private witness (b | `private_xoree`)
///  - 1 constant        (c | `result`)
/// such that: a ^ b = c. 🧙
#[derive(Clone, Copy)]
pub struct XorRelation {
    pub public_xoree: u8,
    pub private_xoree: u8,
    pub result: u8,
}

impl XorRelation {
    pub fn new(public_xoree: u8, private_xoree: u8, result: u8) -> Self {
        Self {
            public_xoree,
            private_xoree,
            result,
        }
    }
}

impl ConstraintSynthesizer<ConstraintF> for XorRelation {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<ConstraintF>,
    ) -> ark_relations::r1cs::Result<()> {
        let public_xoree = UInt8::new_input(ark_relations::ns!(cs, "public_summand"), || {
            Ok(&self.public_xoree)
        })?;
        let private_xoree = UInt8::new_witness(ark_relations::ns!(cs, "private_summand"), || {
            Ok(&self.private_xoree)
        })?;
        let result = UInt8::new_constant(ark_relations::ns!(cs, "result"), &self.result)?;

        let xor = UInt8::xor(&public_xoree, &private_xoree)?;
        xor.enforce_equal(&result)
    }
}
