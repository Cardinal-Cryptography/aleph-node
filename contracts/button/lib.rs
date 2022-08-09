#![cfg_attr(not(feature = "std"), no_std)]

use core::mem::swap;

use access_control::{traits::AccessControlled, Role};
use ink_env::{
    call::{build_call, Call, ExecutionInput, Selector},
    AccountId, DefaultEnvironment, Environment, Error as InkEnvError,
};
use ink_lang as ink;
use ink_prelude::{format, string::String, vec};
use ink_storage::traits::{SpreadAllocate, SpreadLayout};
use openbrush::contracts::psp22::PSP22Error;

pub type BlockNumber = <ButtonGameEnvironment as ink_env::Environment>::BlockNumber;
pub type Balance = <ButtonGameEnvironment as ink_env::Environment>::Balance;
pub type ButtonResult<T> = core::result::Result<T, GameError>;

pub enum ButtonGameEnvironment {}

impl Environment for ButtonGameEnvironment {
    const MAX_EVENT_TOPICS: usize = <DefaultEnvironment as Environment>::MAX_EVENT_TOPICS;

    type AccountId = <DefaultEnvironment as Environment>::AccountId;
    type Balance = <DefaultEnvironment as Environment>::Balance;
    type Hash = <DefaultEnvironment as Environment>::Hash;
    type BlockNumber = u64;
    type Timestamp = <DefaultEnvironment as Environment>::Timestamp;
    type ChainExtension = <DefaultEnvironment as Environment>::ChainExtension;
}

/// GameError types
#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum GameError {
    /// Returned if reset is called before the deadline
    BeforeDeadline,
    /// Returned if button is pressed after the deadline
    AfterDeadline,
    /// Returned if a call is made from an account with missing access control priviledges
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

/// Game contracts storage
#[derive(Debug, SpreadLayout, SpreadAllocate, Default)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
)]
pub struct ButtonData {
    /// How long does TheButton live for?
    pub button_lifetime: BlockNumber,
    /// stores the last account that pressed The Button
    pub last_presser: Option<AccountId>,
    /// block number of the last press
    pub last_press: Option<BlockNumber>,
    /// counter for the number of presses
    pub presses: u128,
    /// AccountId of the PSP22 ButtonToken instance on-chain
    pub reward_token: AccountId,
    /// Account ID of the ticket token
    pub ticket_token: AccountId,
    /// access control contract
    pub access_control: AccountId,
}

/// Provides default implementations of the games API to be called inside IButtonGame trait methods
///
/// Implementing contract needs to return ButtonData read-only and mutably
/// as well as implement `score`: the logic that based on the current block number and the contract storage state returns a users score.
/// Remaining methods have default implementations that can be overriden as needed.
///
/// NOTE: no contract events are being emitted, so the implementing contract is responsible for defining and emitting those.
pub trait ButtonGame {
    /// Getter for the button data
    fn get(&self) -> &ButtonData;

    /// Mutable getter for the button data
    fn get_mut(&mut self) -> &mut ButtonData;

    /// Logic for calculating user score given the particular games rules
    fn score(&self, now: BlockNumber) -> Balance;

    /// Logic for calculating pressiah score
    ///
    /// By defaul the pressiah score is defined as k * sqrt(k)
    /// where k is the number of players that participated until the button has died
    /// Can be overriden to some other custom calculation
    fn pressiah_score(&self) -> Balance {
        let presses = self.get().presses;
        (presses * num::integer::sqrt(presses)) as Balance
    }

    fn is_dead(&self, now: BlockNumber) -> bool {
        now > self.deadline(now)
    }

    fn deadline(&self, now: BlockNumber) -> BlockNumber {
        let ButtonData {
            last_press,
            button_lifetime,
            ..
        } = self.get();
        last_press.unwrap_or(now) + button_lifetime
    }

    fn access_control(&self) -> AccountId {
        self.get().access_control
    }

    fn ticket_token(&self) -> AccountId {
        self.get().ticket_token
    }

    fn set_access_control(
        &mut self,
        new_access_control: AccountId,
        caller: AccountId,
        this: AccountId,
    ) -> ButtonResult<()>
    where
        Self: AccessControlled,
    {
        let required_role = Role::Owner(this);
        self.check_role(caller, required_role)?;
        self.get_mut().access_control = new_access_control;
        Ok(())
    }

    fn last_presser(&self) -> Option<AccountId> {
        self.get().last_presser
    }

    fn reward_token(&self) -> AccountId {
        self.get().reward_token
    }

    fn balance<E>(&self, balance_of_selector: [u8; 4], this: AccountId) -> ButtonResult<Balance>
    where
        E: Environment<AccountId = AccountId>,
    {
        let ticket_token = self.get().ticket_token;
        let balance = build_call::<E>()
            .call_type(Call::new().callee(ticket_token))
            .exec_input(ExecutionInput::new(Selector::new(balance_of_selector)).push_arg(this))
            .returns::<Balance>()
            .fire()?;
        Ok(balance)
    }

    fn transfer_tx<E>(
        &self,
        transfer_selector: [u8; 4],
        to: AccountId,
        value: Balance,
    ) -> Result<Result<(), PSP22Error>, InkEnvError>
    where
        E: Environment<AccountId = AccountId>,
    {
        build_call::<E>()
            .call_type(Call::new().callee(self.get().ticket_token))
            .exec_input(
                ExecutionInput::new(Selector::new(transfer_selector))
                    .push_arg(to)
                    .push_arg(value)
                    .push_arg(vec![0x0]),
            )
            .returns::<Result<(), PSP22Error>>()
            .fire()
    }

    fn mint_tx<E>(
        &self,
        mint_to_selector: [u8; 4],
        to: AccountId,
        amount: Balance,
    ) -> Result<Result<(), PSP22Error>, InkEnvError>
    where
        E: Environment<AccountId = AccountId>,
    {
        build_call::<E>()
            .call_type(Call::new().callee(self.get().reward_token))
            .exec_input(
                ExecutionInput::new(Selector::new(mint_to_selector))
                    .push_arg(to)
                    .push_arg(amount),
            )
            .returns::<Result<(), PSP22Error>>()
            .fire()
    }

    fn check_role(&self, account: AccountId, role: Role) -> ButtonResult<()>
    where
        Self: AccessControlled,
    {
        <Self as AccessControlled>::check_role(
            self.get().access_control,
            account,
            role,
            |why: InkEnvError| {
                GameError::ContractCall(format!("Calling access control has failed: {:?}", why))
            },
            |role: Role| GameError::MissingRole(format!("{:?}", role)),
        )
    }

    fn press<E>(
        &mut self,
        transfer_selector: [u8; 4],
        mint_to_selector: [u8; 4],
        now: BlockNumber,
        caller: AccountId,
        this: AccountId,
    ) -> ButtonResult<()>
    where
        E: Environment<AccountId = AccountId>,
    {
        if self.is_dead(now) {
            return Err(GameError::AfterDeadline);
        }

        let ButtonData { presses, .. } = self.get();

        // transfers 1 ticket token to self
        // tx will fail if user did not give allowance to the game contract
        // or does not have enough balance
        self.transfer_tx::<E>(transfer_selector, this, 1)??;

        let root_key = ::ink_primitives::Key::from([0x00; 32]);
        let mut state = ::ink_storage::traits::pull_spread_root::<ButtonData>(&root_key);

        let score = self.score(now);

        // mints reward tokens to pay out the reward
        // contract needs to have a Minter role on the reward token contract
        self.mint_tx::<E>(mint_to_selector, caller, score)??;

        state.presses = presses + 1;
        state.last_presser = Some(caller);
        state.last_press = Some(now);

        swap(self.get_mut(), &mut state);

        Ok(())
    }

    /// Reset the game
    ///
    /// Erases the storage and pays award to the Pressiah
    /// Can be called by any account on behalf of a player
    /// Can only be called after button's deadline
    fn reset<E>(&mut self, now: BlockNumber, mint_to_selector: [u8; 4]) -> ButtonResult<()>
    where
        E: Environment<AccountId = AccountId>,
    {
        let ButtonData { last_presser, .. } = self.get();

        if !self.is_dead(now) {
            return Err(GameError::BeforeDeadline);
        }

        // reward the Pressiah
        if let Some(pressiah) = last_presser {
            let reward = self.pressiah_score();
            self.mint_tx::<E>(mint_to_selector, *pressiah, reward)??;
        };

        // zero the counters in storage
        let root_key = ::ink_primitives::Key::from([0x00; 32]);
        let mut state = ::ink_storage::traits::pull_spread_root::<ButtonData>(&root_key);

        state.presses = 0;
        state.last_presser = None;
        state.last_press = None;
        swap(self.get_mut(), &mut state);

        Ok(())
    }
}

/// Contract trait definition
///
/// This trait defines the game's API
/// You will get default implementations of the matching methods by impl ButtonGame trait
#[ink::trait_definition]
pub trait IButtonGame {
    /// Button press logic
    #[ink(message)]
    fn press(&mut self) -> ButtonResult<()>;

    /// Returns the buttons status
    #[ink(message)]
    fn is_dead(&self) -> bool;

    /// Returns the current deadline
    ///
    /// Deadline is the block number at which the game will end if there are no more participants
    #[ink(message)]
    fn deadline(&self) -> BlockNumber;

    /// Returns the current Pressiah
    #[ink(message)]
    fn last_presser(&self) -> Option<AccountId>;

    /// Returns the current access control contract address
    #[ink(message)]
    fn access_control(&self) -> AccountId;

    /// Returns address of the game's reward token
    #[ink(message)]
    fn reward_token(&self) -> AccountId;

    /// Returns address of the game's ticket token
    #[ink(message)]
    fn ticket_token(&self) -> AccountId;

    /// Returns then number of ticket tokens in the game contract
    #[ink(message)]
    fn balance(&self) -> ButtonResult<Balance>;

    /// Resets the game
    #[ink(message)]
    fn reset(&mut self) -> ButtonResult<()>;

    /// Sets new access control contract address
    ///
    /// Should only be called by the contract owner
    /// Implementing contract is responsible for setting up proper AccessControl
    #[ink(message)]
    fn set_access_control(&mut self, access_control: AccountId) -> ButtonResult<()>;

    /// Terminates the contract
    ///
    /// Should only be called by the contract Owner
    #[ink(message)]
    fn terminate(&mut self) -> ButtonResult<()>;
}
