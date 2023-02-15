use liminal_ark_relation_macro::snark_relation;

/// This relation showcases how to use Poseidon in r1cs circuits
#[snark_relation]
mod dummy_module {

    use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget};
    use ark_relations::ns;
    use liminal_ark_poseidon::circuit;

    use crate::{environment::FpVar, preimage::FrontendHash, shielder::convert_hash, CircuitField};

    /// Preimage relation : H(preimage)=hash
    /// where:
    /// - hash : public input
    /// - preimage : private witness
    #[relation_object_definition]
    struct PreimageRelation {
        /// private witness
        #[private_input]
        pub preimage: CircuitField,
        /// public input
        #[public_input(
            frontend_type = "FrontendHash",
            parse_with = "convert_hash"
            // serialize_with = "flatten_sequence"
        )]
        pub hash: CircuitField,
    }

    #[circuit_definition]
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<CircuitField>,
    ) -> Result<(), SynthesisError> {
        let preimage = FpVar::new_witness(ns!(cs, "preimage"), || self.preimage())?;
        let hash = FpVar::new_input(ns!(cs, "hash"), || self.hash())?;
        let hash_result = circuit::one_to_one_hash(cs, [preimage])?;

        hash.enforce_equal(&hash_result)?;

        Ok(())
    }

    // #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    // pub struct PreimageRelation<S: State> {
    //     // private witness
    //     pub preimage: Option<CircuitField>,
    //     // public input
    //     pub hash: Option<CircuitField>,
    //     _phantom: PhantomData<S>,
    // }

    // impl<S: WithPublicInput> GetPublicInput<CircuitField> for PreimageRelation<S> {
    //     fn public_input(&self) -> Vec<CircuitField> {
    //         vec![self
    //             .hash
    //             .expect("Circuit should have public input assigned")]
    //     }
    // }
}
