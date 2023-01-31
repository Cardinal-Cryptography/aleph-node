//! This module provides 'tangling' - some cheap substitute for real hash function.
//!
//! Tangling is a function that takes in a sequence of bytes (either raw bytes (`tangle`) or as
//! field bytes gadgets (`tangle_in_field`)) and manipulates it. It operates in two steps, as
//! follows:
//!  1.1 For every chunk of length `BASE_LENGTH` we compute suffix sums.
//!  1.2 We build a binary tree over these chunks.
//!  1.3 We go bottom-to-top and in every intermediate node we:
//!      1.3.1 swap the halves
//!      1.3.2 compute prefix products
//!  2.1 Given a new mangled sequence of `n` elements we squash it `SQUASH_FACTOR` times, i.e. we
//!      take chunks of length `SQUASH_FACTOR` and reduce them to a single byte by xoring.
//!
//! Note, it is **not** hiding like any hashing function.
//!
//! This module exposes two implementations of tangling: `tangle` and `tangle_in_field`. They are
//! semantically equivalent, but they just operate on different element types.
//!
//! All the index intervals used here are closed-open, i.e. they are in form `[a, b)`, which means
//! that we consider indices `a`, `a+1`, ..., `b-1`. We also use 0-based indexing.

use std::ops::{Add, AddAssign, Mul};

use ark_ff::{BigInteger, ToConstraintField, Zero};
use ark_r1cs_std::{
    alloc::AllocVar, fields::FieldVar, R1CSVar, ToBytesGadget, ToConstraintFieldGadget,
};
use ark_relations::{ns, r1cs::SynthesisError};

use super::types::ByteVar;
use crate::{environment::FpVar, CircuitField};

/// Bottom-level chunk length.
const BASE_LENGTH: usize = 4;
/// Tangling operates on fixed-sized arrays with 128 elements.
const EXPAND_TO: usize = 128;

/// Entangle `input` into single `FpVar`.
///
/// For circuit use only.
pub(super) fn tangle_in_field(input: &[FpVar]) -> Result<FpVar, SynthesisError> {
    let mut input_expanded = input
        .iter()
        .cycle()
        .take(EXPAND_TO)
        .cloned()
        .collect::<Vec<_>>();

    _tangle_in_field(&mut input_expanded, 0, EXPAND_TO)?;

    Ok(input_expanded.into_iter().reduce(|a, b| a.add(b)).unwrap())
}

/// Recursive and index-bounded implementation of the first step of the `tangle` procedure.
fn _tangle_in_field(elems: &mut [FpVar], low: usize, high: usize) -> Result<(), SynthesisError> {
    // Bottom level case: computing suffix sums of inverses. We have to do some loop-index
    // boilerplate, because Rust doesn't support decreasing range iteration.
    let cs = elems[0].cs();

    if high - low <= BASE_LENGTH {
        let mut i = high - 2;
        loop {
            elems[i] = (&elems[i] + &elems[i + 1]) * FpVar::constant(CircuitField::from(i as u64));

            if i == low {
                break;
            } else {
                i -= 1
            }
        }
    } else {
        // We are in some inner node of the virtual binary tree.
        //
        // We start by recursive call to both halves, so that we proceed in a bottom-top manner.
        let mid = (low + high) / 2;
        _tangle_in_field(elems, low, mid)?;
        _tangle_in_field(elems, mid, high)?;

        // Swapping the halves.
        // for i in low..mid {
        //     let temp = elems[i].clone();
        //     elems[i] = elems[i + mid - low].clone();
        //     elems[i + mid - low] = temp;
        // }

        // Prefix products.
        // for i in low + 1..high {
        // elemes[i] = ByteVar::constant(
        //     u8::overflowing_mul(
        //         elemes[i].value().unwrap_or_default(),
        //         elemes[i - 1].value().unwrap_or_default(),
        //     )
        //     .0,
        // )
        // }
    }
    Ok(())
}

/// Tangle elements of `bytes`.
pub fn tangle(input: &[CircuitField]) -> CircuitField {
    let mut input_expanded = input
        .iter()
        .cycle()
        .take(EXPAND_TO)
        .cloned()
        .collect::<Vec<_>>();

    _tangle(&mut input_expanded, 0, EXPAND_TO);

    input_expanded.into_iter().sum()
}

/// Recursive and index-bounded implementation of the first step of the `tangle` procedure.
///
/// For detailed description, see `_tangle_in_field`.
fn _tangle(elems: &mut [CircuitField], low: usize, high: usize) {
    if high - low <= BASE_LENGTH {
        let mut i = high - 2;
        loop {
            elems[i] = (elems[i] + elems[i + 1]) * CircuitField::from(i as u64);

            if i == low {
                break;
            } else {
                i -= 1
            }
        }
    } else {
        let mid = (low + high) / 2;
        _tangle(elems, low, mid);
        _tangle(elems, mid, high);

        // for i in low..mid {
        //     bytes.swap(i, i + mid - low);
        // }
        //
        // for i in low + 1..high {
        //     bytes[i] = u8::overflowing_mul(bytes[i], bytes[i - 1]).0;
        // }
    }
}

#[cfg(test)]
mod tests {
    use ark_ff::Zero;
    use ark_r1cs_std::{fields::FieldVar, R1CSVar};

    use crate::{
        environment::FpVar,
        shielder::tangle::{tangle, tangle_in_field},
        CircuitField,
    };

    #[test]
    fn tangling_is_homomorphic() {
        let input = vec![
            CircuitField::from(0u64),
            CircuitField::from(100u64),
            CircuitField::from(17u64),
            CircuitField::from(19u64),
        ];
        let tangled = tangle(&input);

        let input_in_field = input.into_iter().map(FpVar::constant).collect::<Vec<_>>();
        let tangled_in_field = tangle_in_field(&input_in_field).unwrap();

        assert_eq!(tangled, tangled_in_field.value().unwrap());
    }

    #[test]
    fn tangles_to_non_zero() {
        let input = vec![CircuitField::zero(); 128];

        let tangled = tangle(&input);
        assert!(tangled.0 .0.into_iter().filter(|b| b.is_zero()).count() <= 1);
    }
}
