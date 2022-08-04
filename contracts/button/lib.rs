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
use openbrush::contracts::psp22::PSP22Error;

pub type BlockNumber = <ButtonGameEnvironment as ink_env::Environment>::BlockNumber;
// scores are denominated in block numbers
pub type Score = BlockNumber;
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
    /// Returned if given account already pressed The Button
    AlreadyParticipated,
    /// Returned if death is called before the deadline
    BeforeDeadline,
    /// Returned if button is pressed after the deadline
    AfterDeadline,
    /// Returned if given accunt has already had its reward paid out
    AlreadyCLaimed,
    /// Account not whitelisted to play
    NotWhitelisted,
    /// Returned if a call is made from an account with missing access control priviledges
    MissingRole(String),
    /// Returned whenever there was already a press tx recorded in this block
    BetterLuckNextTime,
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
    /// Stores a mapping between user accounts and the number of blocks they extended The Buttons life for
    pub presses: Mapping<AccountId, BlockNumber>,
    /// stores total sum of user scores
    pub total_scores: Score,
    /// stores the last account that pressed The Button
    pub last_presser: Option<AccountId>,
    /// block number of the last press
    pub last_press: Option<BlockNumber>,
    /// AccountId of the ERC20 ButtonToken instance on-chain
    pub game_token: AccountId,
    /// accounts whitelisted to play the game
    pub can_play: Mapping<AccountId, ()>,
    /// stores a set of acounts that already collected their rewards
    pub reward_claimed: Mapping<AccountId, ()>,
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

    fn game_token(&self) -> AccountId {
        self.get().game_token
    }

    fn balance<E>(&self, balance_of_selector: [u8; 4], this: AccountId) -> ButtonResult<Balance>
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
    ) -> Result<Result<(), PSP22Error>, InkEnvError>
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

    fn allow(&mut self, player: AccountId, caller: AccountId, this: AccountId) -> ButtonResult<()>
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
    ) -> ButtonResult<()>
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

    fn disallow(
        &mut self,
        player: AccountId,
        caller: AccountId,
        this: AccountId,
    ) -> ButtonResult<()>
    where
        Self: AccessControlled,
    {
        let required_role = Role::Admin(this);
        self.check_role(caller, required_role)?;
        self.get_mut().can_play.remove(&player);
        Ok(())
    }

    // TODO : add nonce?
    fn press(&mut self, now: BlockNumber, caller: AccountId) -> ButtonResult<()> {
        let ButtonData {
            can_play,
            presses,
            last_press,
            ..
        } = self.get();

        if self.is_dead(now) {
            return Err(GameError::AfterDeadline);
        }

        if presses.get(&caller).is_some() {
            return Err(GameError::AlreadyParticipated);
        }

        if can_play.get(&caller).is_none() {
            return Err(GameError::NotWhitelisted);
        }

        // TODO : instead of this?
        // this is to handle a situation when multiple accounts press at the same time (in the same block)
        // as there can be only one succesfull press recorded per block
        // the users are effectively competing for this one tx
        if let Some(last_press) = last_press {
            if last_press.eq(&now) {
                return Err(GameError::BetterLuckNextTime);
            }
        }

        let root_key = ::ink_primitives::Key::from([0x00; 32]);
        let mut state = ::ink_storage::traits::pull_spread_root::<ButtonData>(&root_key);

        let score = self.score(now);

        state.presses.insert(&caller, &score);
        state.last_presser = Some(caller);
        state.last_press = Some(now);
        state.total_scores += score;
        swap(self.get_mut(), &mut state);

        Ok(())
    }

    /// Pays award to a participant
    ///
    /// Can be called by any account on behalf of a player
    /// Can only be called after button's deadline
    fn claim_reward<E>(
        &mut self,
        now: BlockNumber,
        for_player: AccountId,
        balance_of_selector: [u8; 4],
        transfer_selector: [u8; 4],
        this: AccountId,
    ) -> ButtonResult<u128>
    where
        E: Environment<AccountId = AccountId>,
    {
        let ButtonData {
            reward_claimed,
            last_presser,
            presses,
            total_scores,
            ..
        } = self.get();

        if !self.is_dead(now) {
            return Err(GameError::BeforeDeadline);
        }

        if reward_claimed.get(&for_player).is_some() {
            return Err(GameError::AlreadyCLaimed);
        }

        let mut total_rewards = 0;

        match last_presser {
            None => Ok(0), // there weren't any players
            Some(pressiah) => {
                let total_balance = self.balance::<E>(balance_of_selector, this)?;
                let pressiah_reward = total_balance / 2;
                let remaining_balance = total_balance - pressiah_reward;

                if &for_player == pressiah {
                    // Pressiah gets 50% of supply
                    self.transfer_tx::<E>(transfer_selector, *pressiah, pressiah_reward)??;
                    total_rewards += pressiah_reward;
                }

                // NOTE: in this design the Pressiah gets *both* his/her reward *and* a reward for playing

                if let Some(score) = presses.get(&for_player) {
                    // transfer reward proportional to the score
                    let reward = (score as u128 * remaining_balance) / *total_scores as u128;
                    self.transfer_tx::<E>(transfer_selector, for_player, reward)??;

                    // pressiah is also marked as having made the claim, because his/her score was recorder
                    self.get_mut().reward_claimed.insert(&for_player, &());
                    total_rewards += reward;
                }

                Ok(total_rewards)
            }
        }
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

    /// Pays out the award
    #[ink(message)]
    fn claim_reward(&mut self, for_player: AccountId) -> ButtonResult<()>;

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
    fn balance(&self) -> ButtonResult<Balance>;

    /// Sets new access control contract address
    ///
    /// Should only be called by the contract owner
    /// Implementing contract is responsible for setting up proper AccessControl
    #[ink(message)]
    fn set_access_control(&mut self, access_control: AccountId) -> ButtonResult<()>;

    /// Whitelists given AccountId to participate in the game
    ///
    /// Should only be called by the contracts Admin
    #[ink(message)]
    fn allow(&mut self, player: AccountId) -> ButtonResult<()>;

    /// Whitelists an array of accounts to participate in the game
    ///
    /// Should return an error if called by someone else but the Admin
    #[ink(message)]
    fn bulk_allow(&mut self, players: Vec<AccountId>) -> ButtonResult<()>;

    /// Blacklists given AccountId from participating in the game
    ///
    /// Should return an error if called by someone else but the Admin
    #[ink(message)]
    fn disallow(&mut self, player: AccountId) -> ButtonResult<()>;

    /// Terminates the contract
    ///
    /// Should only be called by the contract Owner
    #[ink(message)]
    fn terminate(&mut self) -> ButtonResult<()>;
}
