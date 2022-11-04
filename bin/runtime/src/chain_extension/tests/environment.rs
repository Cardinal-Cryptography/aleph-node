use std::{
    marker::PhantomData,
    ops::Neg,
    sync::mpsc::{channel, Sender},
};

use super::*;

pub trait FunctionMode {}
pub enum StoreKeyMode {}
pub enum VerifyMode {}
impl FunctionMode for StoreKeyMode {}
impl FunctionMode for VerifyMode {}

pub trait ReadingMode {}
pub enum CorruptedMode {}
pub enum StandardMode {}
impl ReadingMode for CorruptedMode {}
impl ReadingMode for StandardMode {}

trait _Read {
    fn _read(&self, max_len: ByteCount) -> Result<Vec<u8>, DispatchError>;
}

pub struct MockedEnvironment<FM: FunctionMode, RM: ReadingMode> {
    /// Channel to report all charges.
    charging_channel: Sender<RevertibleWeight>,

    /// `Some(_)` iff `RM = CorruptedMode`.
    on_read: Option<Box<dyn Fn()>>,
    /// `Some(_)` iff `RM = StandardMode`.
    content: Option<Vec<u8>>,

    in_len: ByteCount,

    _phantom: PhantomData<(FM, RM)>,
}

impl<FM: FunctionMode> MockedEnvironment<FM, CorruptedMode> {
    pub fn new(
        in_len: ByteCount,
        on_read: Option<Box<dyn Fn()>>,
    ) -> (Self, Receiver<RevertibleWeight>) {
        let (sender, receiver) = channel();
        (
            Self {
                charging_channel: sender,
                on_read,
                content: None,
                in_len,
                _phantom: Default::default(),
            },
            receiver,
        )
    }
}
impl<FM: FunctionMode> _Read for MockedEnvironment<FM, CorruptedMode> {
    fn _read(&self, _max_len: ByteCount) -> Result<Vec<u8>, DispatchError> {
        self.on_read.as_ref().map(|action| action());
        Err(DispatchError::Other("Some error"))
    }
}

impl<FM: FunctionMode> MockedEnvironment<FM, StandardMode> {
    pub fn new(content: Vec<u8>) -> (Self, Receiver<RevertibleWeight>) {
        let (sender, receiver) = channel();
        (
            Self {
                charging_channel: sender,
                on_read: None,
                in_len: content.len() as ByteCount,
                content: Some(content),
                _phantom: Default::default(),
            },
            receiver,
        )
    }
}

impl<FM: FunctionMode> _Read for MockedEnvironment<FM, StandardMode> {
    fn _read(&self, max_len: ByteCount) -> Result<Vec<u8>, DispatchError> {
        let content = self.content.as_ref().unwrap();
        if max_len > self.in_len {
            Ok(content.clone())
        } else {
            Ok(content[..max_len as usize].to_vec())
        }
    }
}

impl<RM: ReadingMode> MockedEnvironment<StoreKeyMode, RM> {
    pub fn approx_key_len(&self) -> ByteCount {
        self.in_len
            .checked_sub(size_of::<VerificationKeyIdentifier>() as ByteCount)
            .unwrap()
    }
}

impl<FM: FunctionMode, RM: ReadingMode> Environment for MockedEnvironment<FM, RM>
where
    MockedEnvironment<FM, RM>: _Read,
{
    type ChargedAmount = Weight;

    fn in_len(&self) -> ByteCount {
        self.in_len
    }

    fn read(&self, max_len: u32) -> Result<Vec<u8>, DispatchError> {
        self._read(max_len)
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
