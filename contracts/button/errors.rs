use access_control::roles::Role;
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
    /// A call to a PSP22 contract has failed
    PSP22Error(PSP22Error),
    /// An interaction with ink! environment has failed
    InkEnvError(String),
    /// Couldn't have retrieved own code hash
    CantRetrieveOwnCodeHash,
}

impl From<PSP22Error> for GameError {
    fn from(e: PSP22Error) -> Self {
        GameError::PSP22Error(e)
    }
}

impl From<InkEnvError> for GameError {
    fn from(e: InkEnvError) -> Self {
        GameError::InkEnvError(format!("{:?}", e))
    }
}
