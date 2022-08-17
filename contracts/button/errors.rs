use ink_env::Error as InkEnvError;
use ink_prelude::{format, string::String};
use openbrush::contracts::psp22::PSP22Error;

/// GameError types
#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum GameError {
    /// Returned if reset is called before the deadline
    BeforeDeadline,
    /// Returned if button is pressed after the deadline
    AfterDeadline,
    /// Returned if a call is made from an account with missing access control privileges
    MissingRole(String),
    /// Returned if a call to another contract has failed
    ContractCall(String),
}

impl From<PSP22Error> for GameError {
    fn from(e: PSP22Error) -> Self {
        match e {
            PSP22Error::Custom(message) => GameError::ContractCall(message),
            PSP22Error::InsufficientBalance => {
                GameError::ContractCall(String::from("PSP22::InsufficientBalance"))
            }
            PSP22Error::InsufficientAllowance => {
                GameError::ContractCall(String::from("PSP22::InsufficientAllowance"))
            }
            PSP22Error::ZeroRecipientAddress => {
                GameError::ContractCall(String::from("PSP22::ZeroRecipientAddress"))
            }
            PSP22Error::ZeroSenderAddress => {
                GameError::ContractCall(String::from("PSP22::ZeroSenderAddress"))
            }
            PSP22Error::SafeTransferCheckFailed(message) => {
                GameError::ContractCall(format!("PSP22::SafeTransferCheckFailed({})", message))
            }
        }
    }
}

impl From<InkEnvError> for GameError {
    fn from(e: InkEnvError) -> Self {
        match e {
            InkEnvError::Decode(_e) => {
                GameError::ContractCall(String::from("Contract call failed due to Decode error"))
            }
            InkEnvError::CalleeTrapped => GameError::ContractCall(String::from(
                "Contract call failed due to CalleeTrapped error",
            )),
            InkEnvError::CalleeReverted => GameError::ContractCall(String::from(
                "Contract call failed due to CalleeReverted error",
            )),
            InkEnvError::KeyNotFound => GameError::ContractCall(String::from(
                "Contract call failed due to KeyNotFound error",
            )),
            InkEnvError::_BelowSubsistenceThreshold => GameError::ContractCall(String::from(
                "Contract call failed due to _BelowSubsistenceThreshold error",
            )),
            InkEnvError::TransferFailed => GameError::ContractCall(String::from(
                "Contract call failed due to TransferFailed error",
            )),
            InkEnvError::_EndowmentTooLow => GameError::ContractCall(String::from(
                "Contract call failed due to _EndowmentTooLow error",
            )),
            InkEnvError::CodeNotFound => GameError::ContractCall(String::from(
                "Contract call failed due to CodeNotFound error",
            )),
            InkEnvError::NotCallable => GameError::ContractCall(String::from(
                "Contract call failed due to NotCallable error",
            )),
            InkEnvError::Unknown => {
                GameError::ContractCall(String::from("Contract call failed due to Unknown error"))
            }
            InkEnvError::LoggingDisabled => GameError::ContractCall(String::from(
                "Contract call failed due to LoggingDisabled error",
            )),
            InkEnvError::EcdsaRecoveryFailed => GameError::ContractCall(String::from(
                "Contract call failed due to EcdsaRecoveryFailed error",
            )),
            #[cfg(any(feature = "std", test, doc))]
            InkEnvError::OffChain(_e) => {
                GameError::ContractCall(String::from("Contract call failed due to OffChain error"))
            }
        }
    }
}
