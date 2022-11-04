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
    on_read: Option<Box<dyn Fn()>>,
    _phantom: PhantomData<M>,
}

impl<M: Mode> InputCorruptedEnvironment<M> {
    pub fn new(
        in_len: ByteCount,
        on_read: Option<Box<dyn Fn()>>,
    ) -> (Self, Receiver<RevertibleWeight>) {
        let (sender, receiver) = channel();
        (
            Self {
                in_len,
                on_read,
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
        self.on_read.as_ref().map(|action| action());
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

/// 'Fully functional' mock.
pub struct MockedEnvironment<M: Mode> {
    charging_channel: Sender<RevertibleWeight>,
    content: Vec<u8>,
    _phantom: PhantomData<M>,
}

impl<M: Mode> MockedEnvironment<M> {
    pub fn new(content: Vec<u8>) -> (Self, Receiver<RevertibleWeight>) {
        let (sender, receiver) = channel();
        (
            Self {
                content,
                charging_channel: sender,
                _phantom: Default::default(),
            },
            receiver,
        )
    }
}

impl MockedEnvironment<StoreKeyMode> {
    pub fn key_len(&self) -> ByteCount {
        self.in_len() - (size_of::<VerificationKeyIdentifier>() as ByteCount)
    }
}

impl<M: Mode> Environment for MockedEnvironment<M> {
    type ChargedAmount = Weight;

    fn in_len(&self) -> ByteCount {
        self.content.len() as ByteCount
    }

    fn read(&self, max_len: u32) -> Result<Vec<u8>, DispatchError> {
        if max_len > self.in_len() {
            Ok(self.content.clone())
        } else {
            Ok(self.content[..max_len as usize].to_vec())
        }
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
