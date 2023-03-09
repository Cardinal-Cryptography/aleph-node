use ark_r1cs_std::{
    alloc::{AllocVar, AllocationMode},
    eq::EqGadget,
};
use ark_relations::{
    ns,
    r1cs::{ConstraintSystemRef, SynthesisError},
};

use crate::{
    environment::FpVar, BackendNote, BackendNullifier, BackendTokenAmount, BackendTokenId,
    BackendTrapdoor, CircuitField,
};

#[derive(Clone, Debug)]
pub struct NoteVar {
    pub token_id: FpVar,
    pub token_amount: FpVar,
    pub trapdoor: FpVar,
    pub nullifier: FpVar,
    pub note: FpVar,
}

#[derive(Clone, Debug)]
pub struct NoteVarBuilder<
    const TOKEN_ID_SET: bool,
    const TOKEN_AMOUNT_SET: bool,
    const TRAPDOOR_SET: bool,
    const NULLIFIER_SET: bool,
    const NOTE_SET: bool,
> {
    token_id: Option<FpVar>,
    token_amount: Option<FpVar>,
    trapdoor: Option<FpVar>,
    nullifier: Option<FpVar>,
    note: Option<FpVar>,
    cs: ConstraintSystemRef<CircuitField>,
}

impl NoteVarBuilder<false, false, false, false, false> {
    pub fn new(cs: ConstraintSystemRef<CircuitField>) -> Self {
        NoteVarBuilder {
            token_id: None,
            token_amount: None,
            trapdoor: None,
            nullifier: None,
            note: None,
            cs,
        }
    }
}

type Result<T> = core::result::Result<T, SynthesisError>;

impl<const _1: bool, const _2: bool, const _3: bool, const _4: bool>
    NoteVarBuilder<false, _1, _2, _3, _4>
{
    pub fn with_token_id(
        self,
        token_id: Result<&BackendTokenId>,
        mode: AllocationMode,
    ) -> Result<NoteVarBuilder<true, _1, _2, _3, _4>> {
        let token_id = FpVar::new_variable(ns!(self.cs, "token id"), || token_id, mode)?;
        Ok(self.with_token_id_var(token_id))
    }

    pub fn with_token_id_var(self, token_id: FpVar) -> NoteVarBuilder<true, _1, _2, _3, _4> {
        NoteVarBuilder {
            token_id: Some(token_id),
            token_amount: self.token_amount,
            trapdoor: self.trapdoor,
            nullifier: self.nullifier,
            note: self.note,
            cs: self.cs,
        }
    }
}

impl<const _1: bool, const _2: bool, const _3: bool, const _4: bool>
    NoteVarBuilder<_1, false, _2, _3, _4>
{
    pub fn with_token_amount(
        self,
        amount: Result<&BackendTokenAmount>,
        mode: AllocationMode,
    ) -> Result<NoteVarBuilder<_1, true, _2, _3, _4>> {
        let amount = FpVar::new_variable(ns!(self.cs, "token amount"), || amount, mode)?;
        Ok(self.with_token_amount_var(amount))
    }

    pub fn with_token_amount_var(self, amount: FpVar) -> NoteVarBuilder<_1, true, _2, _3, _4> {
        NoteVarBuilder {
            token_id: self.token_id,
            token_amount: Some(amount),
            trapdoor: self.trapdoor,
            nullifier: self.nullifier,
            note: self.note,
            cs: self.cs,
        }
    }
}

impl<const _1: bool, const _2: bool, const _3: bool, const _4: bool>
    NoteVarBuilder<_1, _2, false, _3, _4>
{
    pub fn with_trapdoor(
        self,
        trapdoor: Result<&BackendTrapdoor>,
        mode: AllocationMode,
    ) -> Result<NoteVarBuilder<_1, _2, true, _3, _4>> {
        let trapdoor = FpVar::new_variable(ns!(self.cs, "trapdoor"), || trapdoor, mode)?;
        Ok(self.with_trapdoor_var(trapdoor))
    }

    pub fn with_trapdoor_var(self, trapdoor: FpVar) -> NoteVarBuilder<_1, _2, true, _3, _4> {
        NoteVarBuilder {
            token_id: self.token_id,
            token_amount: self.token_amount,
            trapdoor: Some(trapdoor),
            nullifier: self.nullifier,
            note: self.note,
            cs: self.cs,
        }
    }
}

impl<const _1: bool, const _2: bool, const _3: bool, const _4: bool>
    NoteVarBuilder<_1, _2, _3, false, _4>
{
    pub fn with_nullifier(
        self,
        nullifier: Result<&BackendNullifier>,
        mode: AllocationMode,
    ) -> Result<NoteVarBuilder<_1, _2, _3, true, _4>> {
        let nullifier = FpVar::new_variable(ns!(self.cs, "nullifier"), || nullifier, mode)?;
        Ok(self.with_nullifier_var(nullifier))
    }

    pub fn with_nullifier_var(self, nullifier: FpVar) -> NoteVarBuilder<_1, _2, _3, true, _4> {
        NoteVarBuilder {
            token_id: self.token_id,
            token_amount: self.token_amount,
            trapdoor: self.trapdoor,
            nullifier: Some(nullifier),
            note: self.note,
            cs: self.cs,
        }
    }
}

impl<const _1: bool, const _2: bool, const _3: bool, const _4: bool>
    NoteVarBuilder<_1, _2, _3, _4, false>
{
    pub fn with_note(
        self,
        note: Result<&BackendNote>,
        mode: AllocationMode,
    ) -> Result<NoteVarBuilder<_1, _2, _3, _4, true>> {
        let note = FpVar::new_variable(ns!(self.cs, "note"), || note, mode)?;
        Ok(self.with_note_var(note))
    }

    pub fn with_note_var(self, note: FpVar) -> NoteVarBuilder<_1, _2, _3, _4, true> {
        NoteVarBuilder {
            token_id: self.token_id,
            token_amount: self.token_amount,
            trapdoor: self.trapdoor,
            nullifier: self.nullifier,
            note: Some(note),
            cs: self.cs,
        }
    }
}

impl NoteVarBuilder<true, true, true, true, true> {
    /// Verify that `note` is indeed the result of hashing `(token_id, token_amount, trapdoor,
    /// nullifier)`. If so, return `NoteVar` holding all components.
    pub fn build(self) -> Result<NoteVar> {
        let note = NoteVar {
            token_id: self.token_id.unwrap(),
            token_amount: self.token_amount.unwrap(),
            trapdoor: self.trapdoor.unwrap(),
            nullifier: self.nullifier.unwrap(),
            note: self.note.unwrap(),
        };

        let hash = liminal_ark_poseidon::circuit::four_to_one_hash(
            self.cs,
            [
                note.token_id.clone(),
                note.token_amount.clone(),
                note.trapdoor.clone(),
                note.nullifier.clone(),
            ],
        )?;

        hash.enforce_equal(&note.note)?;

        Ok(note)
    }
}
