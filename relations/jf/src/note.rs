use ark_ff::Zero;
use jf_primitives::circuit::rescue::RescueNativeGadget;
use jf_relation::{Circuit, PlonkCircuit, Variable};

use crate::{
    shielder_types::{convert_hash, Note, Nullifier, TokenAmount, Trapdoor},
    CircuitField, PlonkResult, PublicInput, Relation,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum NoteType {
    Deposit,
    Spend,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct NoteRelation {
    pub note: Note,
    pub nullifier: Nullifier,
    pub token_id_var: Variable,
    pub token_amount: TokenAmount,
    pub trapdoor: Trapdoor,
    pub note_type: NoteType,
}

impl PublicInput for NoteRelation {
    fn public_input(&self) -> Vec<CircuitField> {
        match self.note_type {
            NoteType::Spend => {
                vec![convert_hash(self.nullifier)]
            }
            NoteType::Deposit => {
                vec![self.token_amount.into(), convert_hash(self.note)]
            }
        }
    }
}

impl Relation for NoteRelation {
    fn generate_subcircuit(
        &self,
        circuit: &mut PlonkCircuit<CircuitField>,
    ) -> PlonkResult<Vec<Variable>> {
        // Register inputs.
        let token_amount_var = circuit.create_variable(self.token_amount.into())?;
        let note_var = circuit.create_variable(convert_hash(self.note))?;
        let nullifier_var = circuit.create_variable(convert_hash(self.nullifier))?;
        let trapdoor_var = circuit.create_variable(convert_hash(self.trapdoor))?;

        match self.note_type {
            NoteType::Spend => {
                circuit.set_variable_public(nullifier_var)?;
            }
            NoteType::Deposit => {
                circuit.set_variable_public(token_amount_var)?;
                circuit.set_variable_public(note_var)?;
            }
        }

        // Ensure that the token amount is valid.
        // todo: extract token amount limiting to at least constant, or even better to a function/type
        circuit.enforce_leq_constant(token_amount_var, CircuitField::from(u128::MAX))?;

        let zero_var = circuit.create_constant_variable(CircuitField::zero())?;

        // Check that the note is valid.
        let inputs: [usize; 6] = [
            self.token_id_var,
            token_amount_var,
            trapdoor_var,
            nullifier_var,
            zero_var,
            zero_var,
        ];
        let computed_note_var = RescueNativeGadget::<CircuitField>::rescue_sponge_no_padding(
            circuit,
            inputs.as_slice(),
            1,
        )?[0];

        circuit.enforce_equal(note_var, computed_note_var)?;

        Ok(vec![token_amount_var])
    }
}

#[cfg(test)]
mod tests {
    use jf_relation::{Circuit, PlonkCircuit, Variable};

    use crate::{
        note::{NoteRelation, NoteType},
        shielder_types::compute_note,
        CircuitField, PublicInput, Relation,
    };

    fn note_relation(token_id_var: Variable, note_type: NoteType) -> NoteRelation {
        let token_id = 0;
        let token_amount = 10;
        let trapdoor = [1; 4];
        let nullifier = [2; 4];
        let note = compute_note(token_id, token_amount, trapdoor, nullifier);

        NoteRelation {
            note,
            nullifier,
            token_id_var,
            token_amount,
            trapdoor,
            note_type,
        }
    }

    #[test]
    fn spend_note() {
        let token_id = 0;
        let mut circuit = PlonkCircuit::<CircuitField>::new_turbo_plonk();
        let token_id_var = circuit.create_public_variable(token_id.into()).unwrap();
        let mut public_input = vec![CircuitField::from(token_id)];
        circuit.check_circuit_satisfiability(&public_input).unwrap();

        let relation = note_relation(token_id_var, NoteType::Spend);
        relation.generate_subcircuit(&mut circuit).unwrap();
        public_input.extend(relation.public_input());

        circuit.check_circuit_satisfiability(&public_input).unwrap();
    }

    #[test]
    fn deposit_note() {
        let token_id = 0;
        let mut circuit = PlonkCircuit::<CircuitField>::new_turbo_plonk();
        let token_id_var = circuit.create_public_variable(token_id.into()).unwrap();
        let mut public_input = vec![CircuitField::from(token_id)];
        circuit.check_circuit_satisfiability(&public_input).unwrap();

        let relation = note_relation(token_id_var, NoteType::Deposit);
        relation.generate_subcircuit(&mut circuit).unwrap();
        public_input.extend(relation.public_input());

        circuit.check_circuit_satisfiability(&public_input).unwrap();
    }
}
