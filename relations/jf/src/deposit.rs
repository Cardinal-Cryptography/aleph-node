use jf_primitives::circuit::rescue::RescueNativeGadget;
use jf_relation::{Circuit, PlonkCircuit};

use crate::{
    shielder_types::{convert_hash, Note, Nullifier, TokenAmount, TokenId, Trapdoor},
    CircuitField, Marshall, PlonkResult, Relation,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositRelation {
    public: DepositPublicInput,
    private: DepositPrivateInput,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositPublicInput {
    pub note: Note,
    pub token_id: TokenId,
    pub token_amount: TokenAmount,
}

impl Marshall for DepositPublicInput {
    fn marshall(&self) -> Vec<CircuitField> {
        vec![
            convert_hash(self.note),
            self.token_id.into(),
            self.token_amount.into(),
        ]
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default, Debug)]
pub struct DepositPrivateInput {
    pub trapdoor: Trapdoor,
    pub nullifier: Nullifier,
}

impl Relation for DepositRelation {
    type PublicInput = DepositPublicInput;
    type PrivateInput = DepositPrivateInput;

    fn new(
        public_input: Self::PublicInput,
        private_input: Self::PrivateInput,
    ) -> PlonkResult<Self> {
        Ok(Self {
            public: public_input,
            private: private_input,
        })
    }

    fn generate_subcircuit(&self, circuit: &mut PlonkCircuit<CircuitField>) -> PlonkResult<()> {
        // Register public inputs.
        let note_var = circuit.create_public_variable(convert_hash(self.public.note))?;
        let token_id_var =
            circuit.create_public_variable(CircuitField::from(self.public.token_id))?;
        let token_amount_var =
            circuit.create_public_variable(CircuitField::from(self.public.token_amount))?;

        // Register private inputs.
        let trapdoor_var = circuit.create_variable(convert_hash(self.private.trapdoor))?;
        let nullifier_var = circuit.create_variable(convert_hash(self.private.nullifier))?;

        // Ensure that the token amount is valid.
        // todo: extract token amount limiting to at least constant, or even better to a function/type
        circuit.enforce_leq_constant(token_amount_var, CircuitField::from(u128::MAX))?;

        // Check that the note is valid.
        // todo: move to a common place
        let inputs: [usize; 6] = [
            token_id_var,
            token_amount_var,
            trapdoor_var,
            nullifier_var,
            circuit.zero(),
            circuit.zero(),
        ];
        let computed_note_var =
            RescueNativeGadget::<CircuitField>::rescue_sponge_no_padding(circuit, &inputs[..], 1)?
                [0];

        circuit.enforce_equal(note_var, computed_note_var)?;

        Ok(())
    }
}
