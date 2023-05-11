use ark_ff::BigInteger256;

use crate::CircuitField;

pub type Note = [u64; 4];
pub type Nullifier = [u64; 4];
pub type TokenId = u16;
pub type TokenAmount = u128;
pub type Trapdoor = [u64; 4];

pub fn convert_hash(array: [u64; 4]) -> CircuitField {
    CircuitField::new(BigInteger256::new(array))
}
