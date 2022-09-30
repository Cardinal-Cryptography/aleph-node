#![cfg_attr(not(feature = "std"), no_std)]

mod errors;

use ink_lang as ink;

#[ink::contract]
mod button_game {
    use access_control::{roles::Role, traits::AccessControlled, ACCESS_CONTROL_PUBKEY};
    use game_token::MINT_TO_SELECTOR;
    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        CallFlags, DefaultEnvironment, Error as InkEnvError,
    };
    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_prelude::{format, vec};
    use ink_storage::traits::{PackedLayout, SpreadLayout};
    use openbrush::contracts::psp22::PSP22Error;
    use scale::{Decode, Encode};
    use ticket_token::TRANSFER_FROM_SELECTOR;

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
    }

    /// Event emitted when the finished game is reset and pressiah is rewarded
    #[ink(event)]
    #[derive(Debug)]
    pub struct GameReset {
        when: BlockNumber,
    }

    /// Scoring strategy indicating what kind of reward users get for pressing the button
    #[derive(Debug, Encode, Decode, Clone, Copy, SpreadLayout, PackedLayout, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    pub enum Scoring {
        /// Pressing the button as soon as possible gives the highest reward
        EarlyBirdSpecial,
        /// Pressing the button as late as possible gives the highest reward
        BackToTheFuture,
        /// The reward increases linearly with the number of participants
        ThePressiahCometh,
    }

    /// Game contracts storage
    #[ink(storage)]
    pub struct ButtonGame {
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
        pub access_control: AccountId,
        /// scoring strategy
        pub scoring: Scoring,
    }

    impl AccessControlled for ButtonGame {
        type ContractError = GameError;
    }

    impl ButtonGame {
        #[ink(constructor)]
        pub fn new(
            ticket_token: AccountId,
            reward_token: AccountId,
            button_lifetime: BlockNumber,
            scoring: Scoring,
        ) -> Self {
            let caller = Self::env().caller();
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Called new on a contract with no code hash");
            let required_role = Role::Initializer(code_hash);
            let access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);

            match ButtonGame::check_role(&access_control, &caller, required_role) {
                Ok(_) => Self::init(ticket_token, reward_token, button_lifetime, scoring),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        /// Returns the current deadline
        ///
        /// Deadline is the block number at which the game will end if there are no more participants
        #[ink(message)]
        pub fn deadline(&self) -> BlockNumber {
            self.last_press + self.button_lifetime
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
            self.last_presser
        }

        /// Returns the current access control contract address
        #[ink(message)]
        pub fn access_control(&self) -> AccountId {
            self.access_control
        }

        /// Returns address of the game's reward token
        #[ink(message)]
        pub fn reward_token(&self) -> AccountId {
            self.reward_token
        }

        /// Returns address of the game's ticket token
        #[ink(message)]
        pub fn ticket_token(&self) -> AccountId {
            self.ticket_token
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
            if self.is_dead() {
                return Err(GameError::AfterDeadline);
            }

            let caller = self.env().caller();
            let now = Self::env().block_number();
            let this = self.env().account_id();

            // transfers 1 ticket token from the caller to self
            // tx will fail if user did not give allowance to the game contract
            // or does not have enough balance
            self.transfer_ticket(caller, this, 1u128)??;

            let score = self.score(now);

            // mints reward tokens to pay out the reward
            // contract needs to have a Minter role on the reward token contract
            self.mint_reward(caller, score)??;

            self.presses += 1;
            self.last_presser = Some(caller);
            self.last_press = now;
            self.total_rewards += score;

            Self::emit_event(
                self.env(),
                Event::ButtonPressed(ButtonPressed {
                    by: caller,
                    when: now,
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
            if !self.is_dead() {
                return Err(GameError::BeforeDeadline);
            }

            let now = self.env().block_number();

            // reward the Pressiah
            if let Some(pressiah) = self.last_presser {
                let reward = self.pressiah_score();
                self.mint_reward(pressiah, reward)??;
            };

            self.presses = 0;
            self.last_presser = None;
            self.last_press = now;
            self.total_rewards = 0;

            Self::emit_event(self.env(), Event::GameReset(GameReset { when: now }));
            Ok(())
        }

        /// Sets new access control contract address
        ///
        /// Should only be called by the contract owner
        /// Implementing contract is responsible for setting up proper AccessControl
        #[ink(message)]
        pub fn set_access_control(&mut self, new_access_control: AccountId) -> ButtonResult<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Owner(this);
            ButtonGame::check_role(&self.access_control, &caller, required_role)?;
            self.access_control = new_access_control;
            Ok(())
        }

        /// Terminates the contract
        ///
        /// Should only be called by the contract Owner
        #[ink(message)]
        pub fn terminate(&mut self) -> ButtonResult<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Owner(this);
            ButtonGame::check_role(&self.access_control, &caller, required_role)?;
            self.env().terminate_contract(caller)
        }

        //===================================================================================================

        fn init(
            ticket_token: AccountId,
            reward_token: AccountId,
            button_lifetime: BlockNumber,
            scoring: Scoring,
        ) -> Self {
            let now = Self::env().block_number();
            let deadline = now + button_lifetime;

            let contract = Self {
                access_control: AccountId::from(ACCESS_CONTROL_PUBKEY),
                button_lifetime,
                reward_token,
                ticket_token,
                last_press: now,
                scoring,
                last_presser: None,
                presses: 0,
                total_rewards: 0,
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

        fn check_role(
            access_control: &AccountId,
            account: &AccountId,
            role: Role,
        ) -> ButtonResult<()>
        where
            Self: AccessControlled,
        {
            <Self as AccessControlled>::check_role(
                access_control.clone(),
                account.clone(),
                role,
                |why: InkEnvError| {
                    GameError::InkEnvError(format!("Calling access control has failed: {:?}", why))
                },
                |role: Role| GameError::MissingRole(role),
            )
        }

        fn score(&self, now: BlockNumber) -> Balance {
            match self.scoring {
                Scoring::EarlyBirdSpecial => self.deadline().saturating_sub(now) as Balance,
                Scoring::BackToTheFuture => now.saturating_sub(self.last_press) as Balance,
                Scoring::ThePressiahCometh => (self.presses + 1) as Balance,
            }
        }

        fn pressiah_score(&self) -> Balance {
            (self.total_rewards / 4) as Balance
        }

        fn transfer_ticket(
            &self,
            from: AccountId,
            to: AccountId,
            value: Balance,
        ) -> Result<Result<(), PSP22Error>, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(self.ticket_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TRANSFER_FROM_SELECTOR))
                        .push_arg(from)
                        .push_arg(to)
                        .push_arg(value)
                        .push_arg(vec![0x0]),
                )
                .call_flags(CallFlags::default().set_allow_reentry(true))
                .returns::<Result<(), PSP22Error>>()
                .fire()
        }

        fn mint_reward(
            &self,
            to: AccountId,
            amount: Balance,
        ) -> Result<Result<(), PSP22Error>, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(self.reward_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(MINT_TO_SELECTOR))
                        .push_arg(to)
                        .push_arg(amount),
                )
                .returns::<Result<(), PSP22Error>>()
                .fire()
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<ButtonGame>,
        {
            emitter.emit_event(event);
        }
    }
}
