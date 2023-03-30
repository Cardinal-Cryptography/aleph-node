#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::let_unit_value)]
#![feature(min_specialization)]

mod errors;

#[ink::contract]
pub mod button_game {
    use access_control::{roles::Role, AccessControlRef, ACCESS_CONTROL_PUBKEY};
    #[cfg(feature = "std")]
    use ink::storage::traits::StorageLayout;
    use ink::{
        codegen::{EmitEvent, Env},
        env::{call::FromAccountId, set_code_hash, CallFlags},
        prelude::vec,
        reflect::ContractEventBase,
        ToAccountId,
    };
    use marketplace::marketplace::MarketplaceRef;
    use openbrush::{
        contracts::psp22::{extensions::mintable::PSP22MintableRef, PSP22Ref},
        traits::Storage,
    };
    use scale::{Decode, Encode};
    use shared_traits::{Haltable, HaltableData, HaltableError, Internal};

    use crate::errors::GameError;

    /// Result type
    type ButtonResult<T> = core::result::Result<T, GameError>;

    /// Event type
    type Event = <ButtonGame as ContractEventBase>::Type;

    /// Event emitted when TheButton is created
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonCreated {
        #[ink(topic)]
        reward_token: AccountId,
        #[ink(topic)]
        ticket_token: AccountId,
        start: BlockNumber,
        deadline: BlockNumber,
    }

    /// Event emitted when TheButton is pressed
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonPressed {
        #[ink(topic)]
        by: AccountId,
        when: BlockNumber,
        score: Balance,
    }

    /// Event emitted when the finished game is reset and pressiah is rewarded
    #[ink(event)]
    #[derive(Debug)]
    pub struct GameReset {
        when: BlockNumber,
    }

    #[ink(event)]
    pub struct Halted;

    #[ink(event)]
    pub struct Resumed;

    /// Scoring strategy indicating what kind of reward users get for pressing the button
    #[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub enum Scoring {
        /// Pressing the button as soon as possible gives the highest reward
        EarlyBirdSpecial,
        /// Pressing the button as late as possible gives the highest reward
        BackToTheFuture,
        /// The reward increases linearly with the number of participants
        ThePressiahCometh,
    }

    pub const STORAGE_KEY: u32 = openbrush::storage_unique_key!(Data);

    #[derive(Debug)]
    #[openbrush::upgradeable_storage(STORAGE_KEY)]
    pub struct Data {
        /// How long does TheButton live for?
        pub button_lifetime: BlockNumber,
        /// stores the last account that pressed The Button
        pub last_presser: Option<AccountId>,
        /// block number of the last press, set to current block number at button start/reset
        pub last_press: BlockNumber,
        /// sum of rewards paid to players in the current iteration
        pub total_rewards: u128,
        /// counter for the number of presses
        pub presses: u128,
        /// AccountId of the PSP22 ButtonToken instance on-chain
        pub reward_token: AccountId,
        /// Account ID of the ticket token
        pub ticket_token: AccountId,
        /// access control contract
        pub access_control: AccessControlRef,
        /// ticket marketplace contract
        pub marketplace: MarketplaceRef,
        /// scoring strategy
        pub scoring: Scoring,
        /// current round number
        pub round: u64,
        /// reserved for future updates
        pub _reserved: Option<()>,
    }

    /// Game contracts storage
    #[ink(storage)]
    #[derive(Storage)]
    pub struct ButtonGame {
        data: Data,
        /// is contract in the halted state
        #[storage_field]
        pub halted: HaltableData,
    }

    impl Internal for ButtonGame {
        fn _after_halt(&self) -> Result<(), HaltableError> {
            Self::emit_event(self.env(), Event::Halted(Halted {}));
            Ok(())
        }

        fn _after_resume(&self) -> Result<(), HaltableError> {
            Self::emit_event(self.env(), Event::Resumed(Resumed {}));
            Ok(())
        }

        fn _before_halt(&self) -> Result<(), HaltableError> {
            self.check_role(self.env().caller(), Role::Admin(self.env().account_id()))?;
            Ok(())
        }

        fn _before_resume(&self) -> Result<(), HaltableError> {
            self.check_role(self.env().caller(), Role::Admin(self.env().account_id()))?;
            Ok(())
        }
    }

    impl Haltable for ButtonGame {}

    impl ButtonGame {
        #[ink(constructor)]
        pub fn new(
            ticket_token: AccountId,
            reward_token: AccountId,
            marketplace: AccountId,
            button_lifetime: BlockNumber,
            scoring: Scoring,
        ) -> Self {
            let caller = Self::env().caller();
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Called new on a contract with no code hash");
            let required_role = Role::Initializer(code_hash);
            let access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            let access_control = AccessControlRef::from_account_id(access_control);

            match access_control.has_role(caller, required_role) {
                true => Self::init(
                    access_control,
                    ticket_token,
                    reward_token,
                    marketplace,
                    button_lifetime,
                    scoring,
                ),
                false => panic!("Caller is not allowed to initialize this contract"),
            }
        }

        /// Returns the current deadline
        ///
        /// Deadline is the block number at which the game will end if there are no more participants
        #[ink(message)]
        pub fn deadline(&self) -> BlockNumber {
            self.data.last_press + self.data.button_lifetime
        }

        /// Returns the curent round number
        #[ink(message)]
        pub fn round(&self) -> u64 {
            self.data.round
        }

        /// Returns the buttons status
        #[ink(message)]
        pub fn is_dead(&self) -> bool {
            self.env().block_number() > self.deadline()
        }

        /// Returns the last player who pressed the button.
        /// If button is dead, this is The Pressiah.
        #[ink(message)]
        pub fn last_presser(&self) -> Option<AccountId> {
            self.data.last_presser
        }

        /// Returns the current access control contract address
        #[ink(message)]
        pub fn access_control(&self) -> AccountId {
            self.data.access_control.to_account_id()
        }

        /// Returns address of the game's reward token
        #[ink(message)]
        pub fn reward_token(&self) -> AccountId {
            self.data.reward_token
        }

        /// Returns address of the game's ticket token
        #[ink(message)]
        pub fn ticket_token(&self) -> AccountId {
            self.data.ticket_token
        }

        /// Returns the address of the marketplace for exchanging this game's rewards for tickets.
        #[ink(message)]
        pub fn marketplace(&self) -> AccountId {
            self.data.marketplace.to_account_id()
        }

        /// Returns own code hash
        #[ink(message)]
        pub fn code_hash(&self) -> ButtonResult<Hash> {
            self.env()
                .own_code_hash()
                .map_err(|_| GameError::CantRetrieveOwnCodeHash)
        }

        /// Presses the button
        ///
        /// If called on alive button, instantaneously mints reward tokens to the caller
        #[ink(message)]
        pub fn press(&mut self) -> ButtonResult<()> {
            self.check_halted()?;

            if self.is_dead() {
                return Err(GameError::AfterDeadline);
            }

            let caller = self.env().caller();
            let now = Self::env().block_number();
            let this = self.env().account_id();

            // transfers 1 ticket token from the caller to self
            // tx will fail if user did not give allowance to the game contract
            // or does not have enough balance
            self.transfer_ticket(caller, this, 1u128)?;

            let score = self.score(now);

            // mints reward tokens to pay out the reward
            // contract needs to have a Minter role on the reward token contract
            self.mint_reward(caller, score)?;

            self.data.presses += 1;
            self.data.last_presser = Some(caller);
            self.data.last_press = now;
            self.data.total_rewards += score;

            Self::emit_event(
                self.env(),
                Event::ButtonPressed(ButtonPressed {
                    by: caller,
                    when: now,
                    score,
                }),
            );

            Ok(())
        }

        /// Resets the game
        ///
        /// Erases the storage and pays award to the Pressiah
        /// Can be called by any account on behalf of a player
        /// Can only be called after button's deadline
        #[ink(message)]
        pub fn reset(&mut self) -> ButtonResult<()> {
            self.ensure_dead()?;
            self.reward_pressiah()?;
            self.reset_state()?;
            self.transfer_tickets_to_marketplace()?;
            self.reset_marketplace()
        }

        /// Sets new access control contract address
        ///
        /// Should only be called by the contract Admin
        /// Implementing contract is responsible for setting up proper AccessControl
        #[ink(message)]
        pub fn set_access_control(&mut self, new_access_control: AccountId) -> ButtonResult<()> {
            self.check_role(self.env().caller(), Role::Admin(self.env().account_id()))?;
            self.data.access_control = AccessControlRef::from_account_id(new_access_control);
            Ok(())
        }

        /// Sets button lifetime to a new value
        ///
        /// Can only be called by the contract admin
        #[ink(message)]
        pub fn set_button_lifetime(
            &mut self,
            new_button_lifetime: BlockNumber,
        ) -> ButtonResult<()> {
            self.check_role(self.env().caller(), Role::Admin(self.env().account_id()))?;
            self.data.button_lifetime = new_button_lifetime;
            Ok(())
        }

        /// Terminates the contract
        ///
        /// Should only be called by the contract Admin
        #[ink(message)]
        pub fn terminate(&mut self) -> ButtonResult<()> {
            let caller = self.env().caller();
            self.check_role(caller, Role::Admin(self.env().account_id()))?;
            self.env().terminate_contract(caller)
        }

        /// Upgrades contract code
        #[ink(message)]
        pub fn set_code(&mut self, code_hash: [u8; 32]) -> ButtonResult<()> {
            self.check_role(self.env().caller(), Role::Admin(self.env().account_id()))?;
            set_code_hash(&code_hash)?;
            Ok(())
        }

        //===================================================================================================

        fn init(
            access_control: AccessControlRef,
            ticket_token: AccountId,
            reward_token: AccountId,
            marketplace: AccountId,
            button_lifetime: BlockNumber,
            scoring: Scoring,
        ) -> Self {
            let now = Self::env().block_number();
            let deadline = now + button_lifetime;

            let contract = Self {
                data: Data {
                    access_control,
                    button_lifetime,
                    reward_token,
                    ticket_token,
                    marketplace: MarketplaceRef::from_account_id(marketplace),
                    last_press: now,
                    scoring,
                    last_presser: None,
                    presses: 0,
                    total_rewards: 0,
                    round: 0,
                    _reserved: None,
                },
                halted: HaltableData { halted: false },
            };

            Self::emit_event(
                Self::env(),
                Event::ButtonCreated(ButtonCreated {
                    start: now,
                    deadline,
                    ticket_token,
                    reward_token,
                }),
            );

            contract
        }

        fn reset_state(&mut self) -> ButtonResult<()> {
            let now = self.env().block_number();

            self.data.presses = 0;
            self.data.last_presser = None;
            self.data.last_press = now;
            self.data.total_rewards = 0;
            self.data.round = self
                .data
                .round
                .checked_add(1)
                .ok_or(GameError::Arithmethic)?;

            Self::emit_event(self.env(), Event::GameReset(GameReset { when: now }));
            Ok(())
        }

        fn reward_pressiah(&self) -> ButtonResult<()> {
            if let Some(pressiah) = self.data.last_presser {
                let reward = self.pressiah_score();
                self.mint_reward(pressiah, reward)?;
            };

            Ok(())
        }

        fn ensure_dead(&self) -> ButtonResult<()> {
            if !self.is_dead() {
                Err(GameError::BeforeDeadline)
            } else {
                Ok(())
            }
        }

        fn transfer_tickets_to_marketplace(&self) -> ButtonResult<()> {
            PSP22Ref::transfer_builder(
                &self.data.ticket_token,
                self.data.marketplace.to_account_id(),
                self.held_tickets(),
                vec![],
            )
            .call_flags(CallFlags::default().set_allow_reentry(true))
            .invoke()?;

            Ok(())
        }

        fn held_tickets(&self) -> Balance {
            PSP22Ref::balance_of(&self.data.ticket_token, self.env().account_id())
        }

        fn reset_marketplace(&mut self) -> ButtonResult<()> {
            self.data.marketplace.reset()?;

            Ok(())
        }

        fn check_role(&self, account: AccountId, role: Role) -> ButtonResult<()> {
            if self.data.access_control.has_role(account, role) {
                Ok(())
            } else {
                Err(GameError::MissingRole(role))
            }
        }

        fn score(&self, now: BlockNumber) -> Balance {
            match self.data.scoring {
                Scoring::EarlyBirdSpecial => self.deadline().saturating_sub(now) as Balance,
                Scoring::BackToTheFuture => now.saturating_sub(self.data.last_press) as Balance,
                Scoring::ThePressiahCometh => (self.data.presses + 1) as Balance,
            }
        }

        fn pressiah_score(&self) -> Balance {
            (self.data.total_rewards / 4) as Balance
        }

        fn transfer_ticket(
            &self,
            from: AccountId,
            to: AccountId,
            value: Balance,
        ) -> ButtonResult<()> {
            PSP22Ref::transfer_from_builder(&self.data.ticket_token, from, to, value, vec![])
                .call_flags(CallFlags::default().set_allow_reentry(true))
                .invoke()?;

            Ok(())
        }

        fn mint_reward(&self, to: AccountId, amount: Balance) -> ButtonResult<()> {
            PSP22MintableRef::mint(&self.data.reward_token, to, amount)?;
            Ok(())
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<ButtonGame>,
        {
            emitter.emit_event(event);
        }
    }
}
