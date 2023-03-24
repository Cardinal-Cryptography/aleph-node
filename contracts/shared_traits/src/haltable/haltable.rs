use ink::prelude::{format, string::String};
use openbrush::contracts::psp22::PSP22Error;

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum HaltableError {
    InHaltedState,
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
pub trait Haltable {
    #[ink(message)]
    fn halt(&mut self) -> HaltableResult<()>;

    #[ink(message)]
    fn resume(&mut self) -> HaltableResult<()>;

    #[ink(message)]
    fn is_halted(&self) -> bool;

    #[ink(message)]
    fn check_halted(&self) -> HaltableResult<()>;
}
