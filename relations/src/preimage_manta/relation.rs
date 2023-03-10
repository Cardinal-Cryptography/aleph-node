use liminal_ark_relation_macro::snark_relation;

/// This relation showcases how to use Poseidon in r1cs circuits
#[snark_relation]
mod dummy_module {

    use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget};
    use ark_relations::ns;
    use manta_crypto::{
        arkworks::constraint::R1CS,
        hash::ArrayHashFunction,
        rand::{OsRng, Sample},
    };
    use manta_pay::{
        config::{poseidon::Spec2 as Poseidon2, utxo::InnerHashDomainTag, Compiler},
        crypto::poseidon::hash::Hasher,
    };

    use crate::{
        environment::FpVar,
        preimage::{FrontendHash, FrontendPreimage},
        shielder::convert_hash,
        CircuitField,
    };

    /// Preimage relation : H(preimage)=hash
    /// where:
    /// - hash : public input
    /// - preimage : private witness
    #[relation_object_definition]
    struct PreimageMantaRelation {
        #[private_input(frontend_type = "FrontendPreimage", parse_with = "convert_hash")]
        pub preimage: CircuitField,
        #[private_input(frontend_type = "FrontendPreimage", parse_with = "convert_hash")]
        pub preimage1: CircuitField,
        #[public_input(frontend_type = "FrontendHash", parse_with = "convert_hash")]
        pub hash: CircuitField,
    }

    #[circuit_definition]
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<CircuitField>,
    ) -> Result<(), SynthesisError> {
        let mut rng = OsRng;
        let x = FpVar::new_witness(ns!(cs, "preimage"), || self.preimage())?;
        let y = FpVar::new_witness(ns!(cs, "preimage1"), || self.preimage1())?;
        let hash = FpVar::new_input(ns!(cs, "hash"), || self.hash())?;
        let mut compiler = R1CS::new_unchecked(cs);
        let hasher_circuit =
            Hasher::<Poseidon2, InnerHashDomainTag, 2, R1CS<_>>::sample((), &mut rng);
        let hash_var = hasher_circuit.hash([&x, &y], &mut compiler);

        hash.enforce_equal(&hash)?;

        Ok(())
    }
}
