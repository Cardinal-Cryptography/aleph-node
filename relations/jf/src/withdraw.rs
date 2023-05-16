use ark_bls12_381::Fr;
use jf_primitives::{
    circuit::merkle_tree::{Merkle3AryMembershipProofVar, MerkleTreeGadget, RescueDigestGadget},
    merkle_tree::{
        prelude::RescueSparseMerkleTree, MerkleCommitment, MerkleTreeScheme,
        UniversalMerkleTreeScheme,
    },
};
use jf_relation::{Circuit, PlonkCircuit};
use num_bigint::BigUint;

use crate::{
    note::{NoteGadget, NoteType, SourcedNote},
    shielder_types::{
        convert_account, convert_array, Account, LeafIndex, MerklePath, MerkleRoot, Note,
        Nullifier, TokenAmount, TokenId, Trapdoor,
    },
    CircuitField, PlonkResult, PublicInput, Relation,
};

pub struct WithdrawRelation {
    spend_note: SourcedNote,
    deposit_note: SourcedNote,
    fee: TokenAmount,
    recipient: Account,
    token_amount_out: TokenAmount,
    merkle_root: MerkleRoot,
    merkle_path: MerklePath,
    leaf_index: LeafIndex,
}

impl Default for WithdrawRelation {
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}
#[derive(Default)]
pub struct WithdrawPublicInput {
    pub fee: TokenAmount,
    pub recipient: Account,
    pub token_id: TokenId,
    pub spend_nullifier: Nullifier,
    pub token_amount_out: TokenAmount,
    pub merkle_root: MerkleRoot,
    pub deposit_note: Note,
}
#[derive(Default)]
pub struct WithdrawPrivateInput {
    pub spend_trapdoor: Trapdoor,
    pub deposit_trapdoor: Trapdoor,
    pub deposit_nullifier: Nullifier,
    pub merkle_path: MerklePath,
    pub leaf_index: LeafIndex,
    pub spend_note: Note,
    pub whole_token_amount: TokenAmount,
    pub deposit_token_amount: TokenAmount,
}

impl WithdrawRelation {
    pub fn new(public: WithdrawPublicInput, private: WithdrawPrivateInput) -> Self {
        let spend_note = SourcedNote {
            note: private.spend_note,
            token_id: public.token_id,
            token_amount: private.whole_token_amount,
            trapdoor: private.spend_trapdoor,
            nullifier: public.spend_nullifier,
            note_type: NoteType::Spend,
        };
        let deposit_note = SourcedNote {
            note: public.deposit_note,
            token_id: public.token_id,
            token_amount: public.deposit_token_amount,
            trapdoor: private.deposit_trapdoor,
            nullifier: private.deposit_nullifier,
            note_type: NoteType::Deposit,
        };
        let whole_token_amount = private.whole_token_amount;
        Self {
            spend_note,
            deposit_note,
            fee: public.fee,
            recipient: public.recipient,
            whole_token_amount,
        }
    }
}

impl PublicInput for WithdrawRelation {
    // TODO fix
    fn public_input(&self) -> Vec<CircuitField> {
        let mut public_input = vec![
            self.fee.into(),
            convert_account(self.recipient),
            self.token_amount_out.into(),
        ];
        public_input.extend(self.spend_note.public_input());
        public_input.extend(self.deposit_note.public_input());

        public_input
    }
}

impl Relation for WithdrawRelation {
    fn generate_subcircuit(&self, circuit: &mut PlonkCircuit<CircuitField>) -> PlonkResult<()> {
        let fee_var = circuit.create_public_variable(self.fee.into())?;
        let recipient_var = circuit.create_public_variable(convert_account(self.recipient))?;
        let token_amount_out_var = circuit.create_public_variable(self.token_amount_out.into())?;
        circuit.enforce_leq_constant(token_amount_out_var, CircuitField::from(u128::MAX))?;

        let spend_note_var = circuit.create_note_variable(&self.spend_note)?;
        let whole_token_amount_var = spend_note_var.token_amount_var;
        circuit.enforce_note_preimage(spend_note_var)?;

        let deposit_note_var = circuit.create_note_variable(&self.deposit_note)?;
        let deposit_amount_var = deposit_note_var.token_amount_var;
        circuit.enforce_note_preimage(deposit_note_var)?;

        let token_sum_var = circuit.add(token_amount_out_var, deposit_amount_var)?;
        circuit.enforce_equal(token_sum_var, whole_token_amount_var);

        build_merkle_proof(&mut circuit, convert_array(self.spend_note.note))?;

        Ok(())
    }
}
type MerkleTree = dyn MerkleTreeGadget<
    RescueSparseMerkleTree<BigUint, Fr>,
    MembershipProofVar = Merkle3AryMembershipProofVar,
    DigestGadget = RescueDigestGadget,
>;

fn build_merkle_proof(
    circuit: &mut PlonkCircuit<CircuitField>,
    elem: CircuitField,
) -> PlonkResult<()> {
    let height = 11;
    let uid = BigUint::from(0u64);

    let mt = RescueSparseMerkleTree::from_kv_set(height as usize, &[(uid.clone(), elem)]).unwrap();

    let expected_root = mt.commitment().digest();
    let (retrieved_elem, proof) = mt.lookup(&uid).expect_ok().unwrap();
    assert_eq!(retrieved_elem, elem);
    assert!(mt.verify(&uid, proof.clone()).expect("succeed"));

    let uid_var = circuit.create_variable(uid.into()).unwrap();
    let proof_var = MerkleTree::create_membership_proof_variable(circuit, &proof).unwrap();
    let root_var = MerkleTree::create_root_variable(circuit, expected_root).unwrap();
    MerkleTree::enforce_membership_proof(circuit, uid_var, proof_var, root_var).unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use jf_plonk::{
        proof_system::{PlonkKzgSnark, UniversalSNARK},
        transcript::StandardTranscript,
    };
    use jf_relation::Circuit;

    use crate::{
        deposit::{WithdrawPrivateInput, WithdrawPublicInput, WithdrawRelation},
        generate_srs,
        shielder_types::compute_note,
        Curve, Marshall, Relation,
    };

    fn relation() -> WithdrawRelation {
        let token_id = 0;
        let token_amount = 10;
        let trapdoor = [1; 4];
        let nullifier = [2; 4];
        let note = compute_note(token_id, token_amount, trapdoor, nullifier);

        WithdrawRelation::new(
            WithdrawPublicInput {
                note,
                token_id,
                token_amount,
            },
            WithdrawPrivateInput {
                trapdoor,
                nullifier,
            },
        )
    }

    #[test]
    fn deposit_constraints_correctness() {
        let relation = relation();
        let circuit = WithdrawRelation::generate_circuit(&relation).unwrap();
        circuit
            .check_circuit_satisfiability(&relation.public.marshall())
            .unwrap();
    }

    #[test]
    fn deposit_constraints_incorrectness_with_wrong_note() {
        let mut relation = relation();
        relation.public.note[0] += 1;
        let circuit = WithdrawRelation::generate_circuit(&relation).unwrap();
        assert!(circuit
            .check_circuit_satisfiability(&relation.public.marshall())
            .is_err());
    }

    #[test]
    fn deposit_proving_procedure() {
        let rng = &mut jf_utils::test_rng();
        let srs = generate_srs(10_000, rng).unwrap();

        let (pk, vk) = WithdrawRelation::generate_keys(&srs).unwrap();

        let relation = relation();
        let proof = relation.generate_proof(&pk, rng).unwrap();

        let public_input = relation.public.marshall();

        PlonkKzgSnark::<Curve>::verify::<StandardTranscript>(&vk, &public_input, &proof, None)
            .unwrap();
    }
}
