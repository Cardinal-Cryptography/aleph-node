// use ink::codegen::EmitEvent;
// use core::alloc::Layout;

use access_control::{traits::AccessControlled, Role};
use ink::codegen::EmitEvent;
use ink_env::{
    call::{build_call, Call, ExecutionInput, Selector},
    AccountId, DefaultEnvironment, Error as InkEnvError,
};
use ink_lang as ink;
use ink_prelude::{format, string::String, vec::Vec};
use ink_primitives::KeyPtr;
use ink_storage::{
    traits::{SpreadAllocate, SpreadLayout, StorageLayout},
    Mapping,
};

// pub const TOTAL_SUPPLY_SELECTOR: [u8; 4] = [0, 0, 0, 1];
pub const BALANCE_OF_SELECTOR: [u8; 4] = [0, 0, 0, 2];
// pub const ALLOWANCE_SELECTOR: [u8; 4] = [0, 0, 0, 3];
// pub const TRANSFER_SELECTOR: [u8; 4] = [0, 0, 0, 4];

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

#[derive(
    Debug,
    // PartialEq,
    // scale::Encode,
    // scale::Decode,
    // Clone,
    SpreadLayout,
    // PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout,)
)]
pub struct ButtonData {
    /// How long does TheButton live for?
    pub button_lifetime: u32,
    /// is The Button dead
    pub is_dead: bool,
    /// Stores a mapping between user accounts and the number of blocks they extended The Buttons life for
    pub presses: Mapping<AccountId, u32>,
    /// stores keys to `presses` because Mapping is not an Iterator. Heap-allocated so we might need Map<u32, AccountId> if it grows out of proportion
    pub press_accounts: Vec<AccountId>,
    /// stores total sum of user scores
    pub total_scores: u32,
    /// stores the last account that pressed The Button
    pub last_presser: Option<AccountId>,
    /// block number of the last press
    pub last_press: u32,
    /// AccountId of the ERC20 ButtonToken instance on-chain
    pub button_token: AccountId,
    /// accounts whitelisted to play the game
    pub can_play: Mapping<AccountId, bool>,
    /// access control contract
    pub access_control: AccountId,
}

/// concrete implementation
pub trait ButtonGame {
    /// Getter for the button data
    ///
    /// Needs to be implemented
    fn get(&self) -> &ButtonData;

    fn get_mut(&mut self) -> &mut ButtonData;

    fn press(&mut self) -> Result<()> {
        todo!()
    }

    fn is_dead(&self) -> bool {
        self.get().is_dead
    }

    fn deadline(&self) -> u32 {
        let ButtonData {
            last_press,
            button_lifetime,
            ..
        } = self.get();
        last_press + button_lifetime
    }

    fn score_of(&self, user: AccountId) -> u32 {
        self.get().presses.get(&user).unwrap_or(0)
    }

    fn can_play(&self, user: AccountId) -> bool {
        self.get().can_play.get(&user).unwrap_or(false)
    }

    fn access_control(&self) -> AccountId {
        self.get().access_control
    }

    fn set_access_control(
        &mut self,
        access_control: AccountId,
        caller: AccountId,
        this: AccountId,
    ) -> Result<()>
    where
        Self: AccessControlled,
    {
        let required_role = Role::Owner(this);
        self.check_role(caller, required_role)?;

        self.get_mut().access_control = access_control;
        Ok(())
    }

    fn last_presser(&self) -> Option<AccountId> {
        self.get().last_presser
    }

    fn get_button_token(&self) -> Result<AccountId> {
        Ok(self.get().button_token)
    }

    fn get_balance(&self, balance_of_selector: [u8; 4], this: AccountId) -> Result<Balance> {
        let button_token = self.get().button_token;

        let balance = build_call::<DefaultEnvironment>()
            .call_type(Call::new().callee(button_token))
            .exec_input(ExecutionInput::new(Selector::new(balance_of_selector)).push_arg(this))
            .returns::<Balance>()
            .fire()?;

        Ok(balance)
    }

    fn transfer_tx(
        &self,
        transfer_to_selector: [u8; 4],
        to: AccountId,
        value: u128,
    ) -> core::result::Result<(), InkEnvError> {
        build_call::<DefaultEnvironment>()
            .call_type(Call::new().callee(self.get().button_token))
            .exec_input(
                ExecutionInput::new(Selector::new(transfer_to_selector))
                    .push_arg(to)
                    .push_arg(value),
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

    // TODO: this is harder
    // fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: E) {
    //     emitter.emit_event(event);
    // }

    // TODO: default impl
    fn allow(&mut self, player: AccountId) -> Result<()>;
    // {
    //     let caller = self.env().caller();
    //     let this = self.env().account_id();
    //     let required_role = Role::Admin(this);

    //     self.check_role(caller, required_role)?;

    //     self.can_play.insert(player, &true);
    //     let event = Event::AccountWhitelisted(AccountWhitelisted { player });
    //     Self::emit_event(self.env(), event);
    //     Ok(())
    // }

    // TODO: default impl
    fn bulk_allow(&mut self, players: Vec<AccountId>) -> Result<()>;

    // TODO: default impl
    fn disallow(&mut self, player: AccountId) -> Result<()>;

    // TODO: default impl
    fn terminate(&mut self) -> Result<()>;
}

/// ink trait definition
#[ink::trait_definition]
pub trait IButtonGame {
    /// Button press logic    
    #[ink(message)]
    fn press(&mut self) -> Result<()>;

    /// Returns the buttons status    
    #[ink(message)]
    fn is_dead(&self) -> bool;

    #[ink(message)]
    fn deadline(&self) -> u32;

    #[ink(message)]
    fn score_of(&self, user: AccountId) -> u32;

    #[ink(message)]
    fn can_play(&self, user: AccountId) -> bool;

    #[ink(message)]
    fn access_control(&self) -> AccountId;

    #[ink(message)]
    fn last_presser(&self) -> Option<AccountId>;

    #[ink(message)]
    fn get_button_token(&self) -> Result<AccountId>;

    #[ink(message)]
    fn get_balance(&self) -> Result<Balance>;

    #[ink(message)]
    fn death(&mut self) -> Result<()>;

    #[ink(message)]
    fn set_access_control(&mut self, access_control: AccountId) -> Result<()>;

    #[ink(message)]
    fn allow(&mut self, player: AccountId) -> Result<()>;

    #[ink(message)]
    fn bulk_allow(&mut self, players: Vec<AccountId>) -> Result<()>;

    #[ink(message)]
    fn disallow(&mut self, player: AccountId) -> Result<()>;
}
