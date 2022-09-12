use access_control::Role;
use ink_env::Error as InkEnvError;
use ink_prelude::{format, string::String};
use openbrush::contracts::psp22::PSP22Error;

/// GameError types
#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum GameError {
    /// Reset has been called before the deadline
    BeforeDeadline,
    /// Button has been pressed after the deadline
    AfterDeadline,
    /// Call has been made from an account with missing access control privileges
    MissingRole(Role),
    /// Returned if a call to another contract has failed
    CrossContractCallFailed(String),
}

impl From<PSP22Error> for GameError {
    fn from(e: PSP22Error) -> Self {
        GameError::CrossContractCallFailed(format!("{:?}", e))
    }
}

impl From<InkEnvError> for GameError {
    fn from(e: InkEnvError) -> Self {
        GameError::CrossContractCallFailed(format!("Contract call failed due to {:?}", e))
    }
}
