use ark_ff::PrimeField;
use ark_serialize::CanonicalSerialize;
use ark_std::{vec, vec::Vec};

pub trait GetPublicInput<CircuitField: PrimeField + CanonicalSerialize> {
    fn public_input(&self) -> Vec<CircuitField> {
        vec![]
    }
}

pub(super) mod state {
    #[derive(Clone)]
    pub enum NoInput {}
    #[derive(Clone)]
    pub enum OnlyPublicInput {}
    #[derive(Clone)]
    pub enum FullInput {}

    pub trait State {}
    impl State for NoInput {}
    impl State for OnlyPublicInput {}
    impl State for FullInput {}

    pub trait WithPublicInput: State {}
    impl WithPublicInput for OnlyPublicInput {}
    impl WithPublicInput for FullInput {}
}
