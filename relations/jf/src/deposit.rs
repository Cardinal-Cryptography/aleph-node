use jf_plonk::proof_system::structs::{Proof, ProvingKey, UniversalSrs, VerifyingKey};
use jf_relation::PlonkCircuit;

use crate::{
    shielder_types::{Note, Nullifier, TokenAmount, TokenId, Trapdoor},
    CircuitField, Curve, PlonkResult, Relation,
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

    fn generate_circuit(&self) -> PlonkResult<PlonkCircuit<CircuitField>> {
        todo!()
    }

    fn process_public_input(
        &self,
        public_input: Self::PublicInput,
    ) -> PlonkResult<Vec<CircuitField>> {
        todo!()
    }
}
