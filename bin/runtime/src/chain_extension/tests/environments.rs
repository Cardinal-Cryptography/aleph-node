use std::{
    marker::PhantomData,
    ops::Neg,
    sync::mpsc::{channel, Sender},
};

use super::*;

pub trait Mode {}

pub enum StoreKeyMode {}
pub enum VerifyMode {}

impl Mode for StoreKeyMode {}
impl Mode for VerifyMode {}

/// May signal that there is something to read, but no read can succeed.
pub struct InputCorruptedEnvironment<M: Mode> {
    in_len: ByteCount,
    charging_channel: Sender<RevertibleWeight>,
    _phantom: PhantomData<M>,
}

impl<M: Mode> InputCorruptedEnvironment<M> {
    pub fn new(in_len: ByteCount) -> (Self, Receiver<RevertibleWeight>) {
        let (sender, receiver) = channel();
        (
            Self {
                in_len,
                charging_channel: sender,
                _phantom: Default::default(),
            },
            receiver,
        )
    }
}

impl InputCorruptedEnvironment<StoreKeyMode> {
    pub fn key_len(&self) -> ByteCount {
        self.in_len - (size_of::<VerificationKeyIdentifier>() as ByteCount)
    }
}

impl<M: Mode> Environment for InputCorruptedEnvironment<M> {
    type ChargedAmount = Weight;

    fn in_len(&self) -> ByteCount {
        self.in_len
    }

    fn read(&self, _max_len: u32) -> Result<Vec<u8>, DispatchError> {
        Err(DispatchError::Other("Some error"))
    }

    fn charge_weight(&mut self, amount: Weight) -> Result<Weight, DispatchError> {
        self.charging_channel
            .send(amount as RevertibleWeight)
            .unwrap();
        Ok(amount)
    }

    fn adjust_weight(&mut self, charged: Weight, actual_weight: Weight) {
        self.charging_channel
            .send(((charged - actual_weight) as RevertibleWeight).neg())
            .unwrap();
    }
}
