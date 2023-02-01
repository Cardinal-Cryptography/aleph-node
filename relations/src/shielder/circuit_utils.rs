use core::borrow::Borrow;
#[cfg(feature = "std")]
use std::fmt::{Display, Formatter};

use ark_r1cs_std::{
    alloc::{AllocVar, AllocationMode},
    boolean::Boolean,
    R1CSVar,
};
use ark_relations::r1cs::{Namespace, SynthesisError};

use crate::CircuitField;

#[derive(Clone, Debug)]
pub(super) struct PathShapeVar {
    shape: Vec<Boolean<CircuitField>>,
}

#[cfg(feature = "std")]
impl Display for PathShapeVar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.shape
                .iter()
                .map(|b| b.value().map(|boo| if boo { "left" } else { "right" }))
                .collect::<Vec<_>>()
        )
    }
}

impl PathShapeVar {
    pub(super) fn len(&self) -> usize {
        self.shape.len()
    }

    pub(super) fn at(&self, i: usize) -> &Boolean<CircuitField> {
        &self.shape[i]
    }
}

impl AllocVar<(u8, Result<u64, SynthesisError>), CircuitField> for PathShapeVar {
    fn new_variable<T: Borrow<(u8, Result<u64, SynthesisError>)>>(
        cs: impl Into<Namespace<CircuitField>>,
        f: impl FnOnce() -> Result<T, SynthesisError>,
        mode: AllocationMode,
    ) -> Result<Self, SynthesisError> {
        let ns = cs.into();
        let cs = ns.cs();

        let mut shape = vec![];

        let (path_length, maybe_leaf_index) = *f()?.borrow();

        for i in 0..path_length {
            shape.push(Boolean::new_variable(
                cs.clone(),
                || {
                    let current_index = maybe_leaf_index? / (1 << i);
                    Ok(current_index & 1 != 1 || current_index == 1)
                },
                mode,
            )?);
        }

        Ok(Self { shape })
    }
}
