use ark_ff::BigInteger256;
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
use liminal_ark_poseidon::circuit;

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

impl<S: State> PreimageRelation<S> {
    pub fn new(preimage: Option<CircuitField>, hash: Option<CircuitField>) -> Self {
        PreimageRelation {
            preimage,
            hash,
            _phantom: PhantomData::<S>,
        }
    }
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
    pub fn with_public_input(hash: [u64; 4]) -> Self {
        let backend_hash = CircuitField::new(BigInteger256::new(hash));
        Self {
            preimage: None,
            hash: Some(backend_hash),
            _phantom: PhantomData,
        }
    }
}

impl PreimageRelation<FullInput> {
    pub fn with_full_input(preimage: [u64; 4], hash: [u64; 4]) -> Self {
        let backend_hash = CircuitField::new(BigInteger256::new(hash));
        let backend_preimage = CircuitField::new(BigInteger256::new(preimage));

        Self {
            preimage: Some(backend_preimage),
            hash: Some(backend_hash),
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
