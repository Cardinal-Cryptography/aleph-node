use ink_env::{AccountId, Error as InkEnvError};
use ink_lang as ink;
use ink_prelude::{format, string::String, vec::Vec};
use ink_storage::{traits::SpreadAllocate, Mapping};

pub type Balance = <ink_env::DefaultEnvironment as ink_env::Environment>::Balance;
pub type Result<T> = core::result::Result<T, Error>;

/// Error types
#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    /// Returned if given account already pressed The Button
    AlreadyParticipated,
    /// Returned if button is pressed after the deadline
    AfterDeadline,
    /// Account not whitelisted to play
    NotWhitelisted,
    /// Returned if a call to another contract has failed
    ContractCall(String),
    /// Returned if a call is made from an account with missing access control priviledges
    MissingRole,
}

impl From<InkEnvError> for Error {
    fn from(e: InkEnvError) -> Self {
        match e {
            InkEnvError::Decode(_e) => {
                Error::ContractCall(String::from("Contract call failed due to Decode error"))
            }
            InkEnvError::CalleeTrapped => Error::ContractCall(String::from(
                "Contract call failed due to CalleeTrapped error",
            )),
            InkEnvError::CalleeReverted => Error::ContractCall(String::from(
                "Contract call failed due to CalleeReverted error",
            )),
            InkEnvError::KeyNotFound => Error::ContractCall(String::from(
                "Contract call failed due to KeyNotFound error",
            )),
            InkEnvError::_BelowSubsistenceThreshold => Error::ContractCall(String::from(
                "Contract call failed due to _BelowSubsistenceThreshold error",
            )),
            InkEnvError::TransferFailed => Error::ContractCall(String::from(
                "Contract call failed due to TransferFailed error",
            )),
            InkEnvError::_EndowmentTooLow => Error::ContractCall(String::from(
                "Contract call failed due to _EndowmentTooLow error",
            )),
            InkEnvError::CodeNotFound => Error::ContractCall(String::from(
                "Contract call failed due to CodeNotFound error",
            )),
            InkEnvError::NotCallable => Error::ContractCall(String::from(
                "Contract call failed due to NotCallable error",
            )),
            InkEnvError::Unknown => {
                Error::ContractCall(String::from("Contract call failed due to Unknown error"))
            }
            InkEnvError::LoggingDisabled => Error::ContractCall(String::from(
                "Contract call failed due to LoggingDisabled error",
            )),
            InkEnvError::EcdsaRecoveryFailed => Error::ContractCall(String::from(
                "Contract call failed due to EcdsaRecoveryFailed error",
            )),
            #[cfg(any(feature = "std", test, doc))]
            InkEnvError::OffChain(_e) => {
                Error::ContractCall(String::from("Contract call failed due to OffChain error"))
            }
        }
    }
}

#[ink::trait_definition]
pub trait IButtonGame {
    #[ink(message)]
    fn press(&mut self) -> Result<()>;
}

pub trait ButtonGame {
    fn press(&mut self) -> Result<()> {
        todo!()
    }
}
