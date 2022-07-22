#![cfg_attr(not(feature = "std"), no_std)]

use core::mem::swap;

use access_control::{traits::AccessControlled, Role};
use ink_env::{
    call::{build_call, Call, ExecutionInput, Selector},
    AccountId, DefaultEnvironment, Environment, Error as InkEnvError,
};
use ink_lang as ink;
use ink_prelude::{format, string::String, vec, vec::Vec};
use ink_storage::{
    traits::{SpreadAllocate, SpreadLayout},
    Mapping,
};

pub type BlockNumber = <ButtonGameEnvironment as ink_env::Environment>::BlockNumber;
// scores are denominated in block numbers
pub type Score = BlockNumber;
pub type Balance = <ButtonGameEnvironment as ink_env::Environment>::Balance;
pub type Result<T> = core::result::Result<T, Error>;

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

/// Error types
#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    /// Returned if given account already pressed The Button
    AlreadyParticipated,
    /// Returned if death is called before the deadline
    BeforeDeadline,
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

/// Game contracts storage
#[derive(Debug, SpreadLayout, SpreadAllocate, Default)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
)]
pub struct ButtonData {
    /// How long does TheButton live for?
    pub button_lifetime: BlockNumber,
    /// Stores a mapping between user accounts and the number of blocks they extended The Buttons life for
    pub presses: Mapping<AccountId, BlockNumber>,
    /// stores keys to `presses` because Mapping is not an Iterator. Heap-allocated so we might need Map<int, AccountId> if it grows out of proportion
    pub press_accounts: Vec<AccountId>,
    /// stores total sum of user scores
    pub total_scores: Score,
    /// stores the last account that pressed The Button
    pub last_presser: Option<AccountId>,
    /// block number of the last press
    pub last_press: BlockNumber,
    /// AccountId of the ERC20 ButtonToken instance on-chain
    pub game_token: AccountId,
    /// accounts whitelisted to play the game
    pub can_play: Mapping<AccountId, ()>,
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
    fn score(&self, now: BlockNumber) -> Score;

    fn is_dead(&self, now: BlockNumber) -> bool {
        now > self.deadline()
    }

    fn deadline(&self) -> BlockNumber {
        let ButtonData {
            last_press,
            button_lifetime,
            ..
        } = self.get();
        last_press + button_lifetime
    }

    fn score_of(&self, user: AccountId) -> Score {
        self.get().presses.get(&user).unwrap_or(0)
    }

    fn can_play(&self, user: AccountId) -> bool {
        self.get().can_play.get(&user).is_some()
    }

    fn access_control(&self) -> AccountId {
        self.get().access_control
    }

    fn set_access_control(
        &mut self,
        new_access_control: AccountId,
        caller: AccountId,
        this: AccountId,
    ) -> Result<()>
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

    fn game_token(&self) -> AccountId {
        self.get().game_token
    }

    fn balance<E>(&self, balance_of_selector: [u8; 4], this: AccountId) -> Result<Balance>
    where
        E: Environment<AccountId = AccountId>,
    {
        let game_token = self.get().game_token;
        let balance = build_call::<E>()
            .call_type(Call::new().callee(game_token))
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
    ) -> core::result::Result<(), InkEnvError>
    where
        E: Environment<AccountId = AccountId>,
    {
        build_call::<E>()
            .call_type(Call::new().callee(self.get().game_token))
            .exec_input(
                ExecutionInput::new(Selector::new(transfer_selector))
                    .push_arg(to)
                    .push_arg(value)
                    .push_arg(vec![0x0]),
            )
            .returns::<()>()
            .fire()
    }

    fn check_role(&self, account: AccountId, role: Role) -> Result<()>
    where
        Self: AccessControlled,
    {
        <Self as AccessControlled>::check_role(
            self.get().access_control,
            account,
            role,
            |why: InkEnvError| {
                Error::ContractCall(format!("Calling access control has failed: {:?}", why))
            },
            || Error::MissingRole,
        )
    }

    fn allow(&mut self, player: AccountId, caller: AccountId, this: AccountId) -> Result<()>
    where
        Self: AccessControlled,
    {
        let required_role = Role::Admin(this);
        self.check_role(caller, required_role)?;
        self.get_mut().can_play.insert(player, &());
        Ok(())
    }

    fn bulk_allow(
        &mut self,
        players: Vec<AccountId>,
        caller: AccountId,
        this: AccountId,
    ) -> Result<()>
    where
        Self: AccessControlled,
    {
        let required_role = Role::Admin(this);
        self.check_role(caller, required_role)?;

        for player in players {
            self.get_mut().can_play.insert(player, &());
        }
        Ok(())
    }

    fn disallow(&mut self, player: AccountId, caller: AccountId, this: AccountId) -> Result<()>
    where
        Self: AccessControlled,
    {
        let required_role = Role::Admin(this);
        self.check_role(caller, required_role)?;
        self.get_mut().can_play.remove(&player);
        Ok(())
    }

    fn press(&mut self, now: BlockNumber, caller: AccountId) -> Result<()> {
        let ButtonData {
            can_play, presses, ..
        } = self.get();

        if self.is_dead(now) {
            return Err(Error::AfterDeadline);
        }

        if presses.get(&caller).is_some() {
            return Err(Error::AlreadyParticipated);
        }

        if can_play.get(&caller).is_none() {
            return Err(Error::NotWhitelisted);
        }

        let score = self.score(now);

        let root_key = ::ink_primitives::Key::from([0x00; 32]);
        let mut state = ::ink_storage::traits::pull_spread_root::<ButtonData>(&root_key);

        state.presses.insert(&caller, &score);
        state.press_accounts.push(caller);
        state.last_presser = Some(caller);
        state.last_press = now;
        state.total_scores += score;
        swap(self.get_mut(), &mut state);

        Ok(())
    }

    /// Distibutes awards to the participants
    /// Can only be called after button's deadline
    ///
    /// Will return an Error if called before the deadline
    fn death<E>(
        &self,
        now: BlockNumber,
        balance_of_selector: [u8; 4],
        transfer_selector: [u8; 4],
        this: AccountId,
    ) -> Result<()>
    where
        E: Environment<AccountId = AccountId>,
    {
        if !self.is_dead(now) {
            return Err(Error::BeforeDeadline);
        }

        let ButtonData {
            total_scores,
            last_presser,
            press_accounts,
            presses,
            ..
        } = self.get();

        // if there weren't any players
        if last_presser.is_none() {
            return Ok(());
        }

        let total_balance = self.balance::<E>(balance_of_selector, this)?;

        // Pressiah gets 50% of supply
        let pressiah_reward = total_balance / 2;
        if let Some(pressiah) = last_presser {
            self.transfer_tx::<E>(transfer_selector, *pressiah, pressiah_reward)?;
        }

        let remaining_balance = total_balance - pressiah_reward;
        // rewards are distributed to the participants proportionally to their score
        // let _ =
        press_accounts
            .iter()
            .try_for_each(|account_id| -> Result<()> {
                if let Some(score) = presses.get(account_id) {
                    let reward = (score as u128 * remaining_balance) / *total_scores as u128;
                    // transfer amount
                    return Ok(self.transfer_tx::<E>(transfer_selector, *account_id, reward)?);
                }
                Ok(())
            })

        // Ok(())
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
    fn press(&mut self) -> Result<()>;

    /// End of the game logic
    ///
    /// Distributes the awards
    #[ink(message)]
    fn death(&self) -> Result<()>;

    /// Returns the buttons status
    #[ink(message)]
    fn is_dead(&self) -> bool;

    /// Returns the current deadline
    ///
    /// Deadline is the block number at which the game will end if there are no more participants
    #[ink(message)]
    fn deadline(&self) -> BlockNumber;

    /// Returns the user score
    #[ink(message)]
    fn score_of(&self, user: AccountId) -> Score;

    /// Returns whether given account can play
    #[ink(message)]
    fn can_play(&self, user: AccountId) -> bool;

    /// Returns the current access control contract address
    #[ink(message)]
    fn access_control(&self) -> AccountId;

    /// Returns the current Pressiah
    #[ink(message)]
    fn last_presser(&self) -> Option<AccountId>;

    /// Returns address of the game's ERC20 token
    #[ink(message)]
    fn game_token(&self) -> AccountId;

    /// Returns then game token balance of the game contract
    #[ink(message)]
    fn balance(&self) -> Result<Balance>;

    /// Sets new access control contract address
    ///
    /// Should only be called by the contract owner
    /// Implementing contract is responsible for setting up proper AccessControl
    #[ink(message)]
    fn set_access_control(&mut self, access_control: AccountId) -> Result<()>;

    /// Whitelists given AccountId to participate in the game
    ///
    /// Should only be called by the contracts Admin
    #[ink(message)]
    fn allow(&mut self, player: AccountId) -> Result<()>;

    /// Whitelists an array of accounts to participate in the game
    ///
    /// Should return an error if called by someone else but the Admin
    #[ink(message)]
    fn bulk_allow(&mut self, players: Vec<AccountId>) -> Result<()>;

    /// Blacklists given AccountId from participating in the game
    ///
    /// Should return an error if called by someone else but the Admin
    #[ink(message)]
    fn disallow(&mut self, player: AccountId) -> Result<()>;

    /// Terminates the contract
    ///
    /// Should only be called by the contract Owner
    #[ink(message)]
    fn terminate(&mut self) -> Result<()>;
}
