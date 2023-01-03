// This relation showcases how to use Poseidon in r1cs circuits
use ark_bls12_381::Fq;
use ark_std::{marker::PhantomData, vec::Vec};
use once_cell::sync::Lazy;

use crate::relation::state::State;

// Poseidon paper suggests using domain separation for this, concretely encoding the use case in the capacity element (which is fine as it is 256 bits large and has a lot of bits to fill)
static DOMAIN_SEP: Lazy<Fq> = Lazy::new(|| Fq::from(2137));

/// Preimage relation : H(preimage)=hash
/// where:
/// - hash : public input
/// - preimage : private witness
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PreimageRelation<S: State> {
    // private witness
    pub preimage: Option<Fq>,
    // public input
    pub hash: Option<Fq>,

    _phantom: PhantomData<S>,
}
