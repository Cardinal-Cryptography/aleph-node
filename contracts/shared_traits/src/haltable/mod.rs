use ink::prelude::{format, string::String};
use openbrush::{
    contracts::psp22::PSP22Error,
    traits::{DefaultEnv, Storage, StorageAsMut, StorageAsRef},
};

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum HaltableError {
    InHaltedState,
    NotInHaltedState,
    Custom(String),
}

/// Result type
pub type HaltableResult<T> = Result<T, HaltableError>;

impl From<PSP22Error> for HaltableError {
    fn from(why: PSP22Error) -> Self {
        HaltableError::Custom(format!("{:?}", why))
    }
}

#[ink::trait_definition]
// #[openbrush::trait_definition]
pub trait Haltable {
    #[ink(message)]
    fn halt(&mut self) -> HaltableResult<()>;

    #[ink(message)]
    fn resume(&mut self) -> HaltableResult<()>;

    #[ink(message)]
    fn is_halted(&self) -> bool;
}

pub const STORAGE_KEY: u32 = openbrush::storage_unique_key!(HaltableData);

#[openbrush::upgradeable_storage(STORAGE_KEY)]
pub struct HaltableData {
    pub halted: bool,
}

pub trait DefaultHaltable<T: DefaultEnv>
where
    Self: Sized + Storage<HaltableData>,
{
    fn _after_halt(&self) {}

    fn _after_resume(&self) {}

    fn halt(&mut self) -> HaltableResult<()> {
        if !self.is_halted() {
            <Self as StorageAsMut>::data(self).halted = true;
            self._after_halt();
        }
        Ok(())
    }

    fn resume(&mut self) -> HaltableResult<()> {
        if self.is_halted() {
            <Self as StorageAsMut>::data(self).halted = false;
            self._after_resume();
        }
        Ok(())
    }

    fn is_halted(&self) -> bool {
        <Self as StorageAsRef>::data(self).halted
    }

    fn check_halted(&self) -> HaltableResult<()> {
        if self.is_halted() {
            return Err(HaltableError::InHaltedState);
        }
        Ok(())
    }
}
