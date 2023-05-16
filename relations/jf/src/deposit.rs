use jf_relation::{PlonkCircuit, Variable};

use crate::{
    note::{NoteRelation, NoteType},
    shielder_types::{convert_hash, Note, Nullifier, TokenAmount, TokenId, Trapdoor},
    CircuitField, PlonkResult, ProofSystem, PublicInput, Relation,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositRelation {
    public: DepositPublicInput,
    private: DepositPrivateInput,
}

impl DepositRelation {
    pub fn new(public: DepositPublicInput, private: DepositPrivateInput) -> Self {
        Self { public, private }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositPublicInput {
    pub note: Note,
    pub token_id: TokenId,
    pub token_amount: TokenAmount,
}

impl PublicInput for DepositRelation {
    fn public_input(&self) -> Vec<CircuitField> {
        vec![
            self.public.token_id.into(),
            self.public.token_amount.into(),
            convert_hash(self.public.note),
        ]
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositPrivateInput {
    pub trapdoor: Trapdoor,
    pub nullifier: Nullifier,
}

impl Relation for DepositRelation {
    fn generate_subcircuit(
        &self,
        circuit: &mut PlonkCircuit<CircuitField>,
    ) -> PlonkResult<Vec<Variable>> {
        NoteRelation {
            note: self.public.note,
            nullifier: self.private.nullifier,
            token_id: self.public.token_id,
            token_amount: self.public.token_amount,
            trapdoor: self.private.trapdoor,
            note_type: NoteType::Deposit,
        }
        .generate_subcircuit(circuit)?;

        Ok(Vec::new())
    }
}

impl ProofSystem for DepositRelation {}

#[cfg(test)]
mod tests {
    use jf_plonk::{
        proof_system::{PlonkKzgSnark, UniversalSNARK},
        transcript::StandardTranscript,
    };
    use jf_relation::Circuit;

    use crate::{
        deposit::{DepositPrivateInput, DepositPublicInput, DepositRelation},
        generate_srs,
        shielder_types::compute_note,
        Curve, ProofSystem, PublicInput,
    };

    fn relation() -> DepositRelation {
        let token_id = 0;
        let token_amount = 10;
        let trapdoor = [1; 4];
        let nullifier = [2; 4];
        let note = compute_note(token_id, token_amount, trapdoor, nullifier);

        DepositRelation::new(
            DepositPublicInput {
                note,
                token_id,
                token_amount,
            },
            DepositPrivateInput {
                trapdoor,
                nullifier,
            },
        )
    }

    #[test]
    fn deposit_constraints_correctness() {
        let relation = relation();
        let circuit = DepositRelation::generate_circuit(&relation).unwrap();
        circuit
            .check_circuit_satisfiability(&relation.public_input())
            .unwrap();
    }

    #[test]
    fn deposit_constraints_incorrectness_with_wrong_note() {
        let mut relation = relation();
        relation.public.note[0] += 1;
        let circuit = DepositRelation::generate_circuit(&relation).unwrap();
        assert!(circuit
            .check_circuit_satisfiability(&relation.public_input())
            .is_err());
    }

    #[test]
    fn deposit_proving_procedure() {
        let rng = &mut jf_utils::test_rng();
        let srs = generate_srs(10_000, rng).unwrap();

        let (pk, vk) = DepositRelation::generate_keys(&srs).unwrap();

        let relation = relation();
        let proof = relation.generate_proof(&pk, rng).unwrap();

        let public_input = relation.public_input();

        PlonkKzgSnark::<Curve>::verify::<StandardTranscript>(&vk, &public_input, &proof, None)
            .unwrap();
    }
}
