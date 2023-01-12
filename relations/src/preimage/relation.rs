// This relation showcases how to use Poseidon in r1cs circuits
use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget};
use ark_relations::{
    ns,
    r1cs::{
        ConstraintSynthesizer, ConstraintSystemRef, SynthesisError,
        SynthesisError::AssignmentMissing,
    },
};
use ark_std::{marker::PhantomData, vec, vec::Vec};
use poseidon::circuit;

use crate::{
    environment::FpVar,
    relation::state::{FullInput, NoInput, OnlyPublicInput, State, WithPublicInput},
    CircuitField, GetPublicInput,
};

/// Preimage relation : H(preimage)=hash
/// where:
/// - hash : public input
/// - preimage : private witness
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PreimageRelation<S: State> {
    // private witness
    pub preimage: Option<CircuitField>,
    // public input
    pub hash: Option<CircuitField>,
    _phantom: PhantomData<S>,
}

impl PreimageRelation<NoInput> {
    pub fn without_input() -> Self {
        Self {
            hash: None,
            preimage: None,
            _phantom: PhantomData,
        }
    }
}

impl PreimageRelation<OnlyPublicInput> {
    pub fn with_public_input(hash: CircuitField) -> Self {
        Self {
            preimage: None,
            hash: Some(hash),
            _phantom: PhantomData,
        }
    }
}

impl PreimageRelation<FullInput> {
    pub fn with_full_input(preimage: CircuitField, hash: CircuitField) -> Self {
        Self {
            preimage: Some(preimage),
            hash: Some(hash),
            _phantom: PhantomData,
        }
    }
}

impl<S: State> ConstraintSynthesizer<CircuitField> for PreimageRelation<S> {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<CircuitField>,
    ) -> Result<(), SynthesisError> {
        let preimage = FpVar::new_witness(ns!(cs, "preimage"), || {
            self.preimage.ok_or(AssignmentMissing)
        })?;
        let hash = FpVar::new_input(ns!(cs, "hash"), || self.hash.ok_or(AssignmentMissing))?;
        let hash_result = circuit::one_to_one_hash(cs, preimage)?;

        hash.enforce_equal(&hash_result)?;

        Ok(())
    }
}

impl<S: WithPublicInput> GetPublicInput<CircuitField> for PreimageRelation<S> {
    fn public_input(&self) -> Vec<CircuitField> {
        vec![self
            .hash
            .expect("Circuit should have public input assigned")]
    }
}
