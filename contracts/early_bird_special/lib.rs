#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// This is the EarlyBirdSpecial
///
/// Larger rewards are distributed for engaging in the game as early on as possible:
/// user_score = deadline - now
/// On the other hand ThePressiah (the last player to click) gets 50% of the token pool, which creates two competing strategies.

#[ink::contract(env = button::ButtonGameEnvironment)]
mod early_bird_special {

    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use button::{
        ButtonData, ButtonGame, ButtonGameEnvironment, ButtonResult, GameError, IButtonGame,
    };
    use ink_env::Error as InkEnvError;
    use ink_lang::{
        codegen::{initialize_contract, EmitEvent},
        reflect::ContractEventBase,
    };
    use ink_prelude::format;
    use ink_storage::traits::SpreadAllocate;

    /// Event type
    type Event = <EarlyBirdSpecial as ContractEventBase>::Type;

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

    /// Event emitted when a players reward is claimed
    #[ink(event)]
    #[derive(Debug)]
    pub struct GameReset {
        when: BlockNumber,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct EarlyBirdSpecial {
        data: ButtonData,
    }

    impl AccessControlled for EarlyBirdSpecial {
        type ContractError = GameError;
    }

    impl ButtonGame for EarlyBirdSpecial {
        fn get(&self) -> &ButtonData {
            &self.data
        }

        fn get_mut(&mut self) -> &mut ButtonData {
            &mut self.data
        }

        fn score(&self, now: BlockNumber) -> Balance {
            let deadline = ButtonGame::deadline(self);
            deadline.saturating_sub(now) as Balance
        }
    }

    // because ink! does not allow generics or trait default implementations
    impl IButtonGame for EarlyBirdSpecial {
        #[ink(message)]
        fn is_dead(&self) -> bool {
            let now = self.env().block_number();
            ButtonGame::is_dead(self, now)
        }

        #[ink(message)]
        fn press(&mut self) -> ButtonResult<()> {
            let caller = self.env().caller();
            let now = Self::env().block_number();
            let this = self.env().account_id();

            ButtonGame::press::<ButtonGameEnvironment>(self, now, caller, this)?;

            Self::emit_event(
                self.env(),
                Event::ButtonPressed(ButtonPressed {
                    by: caller,
                    when: now,
                }),
            );

            Ok(())
        }

        #[ink(message)]
        fn reset(&mut self) -> ButtonResult<()> {
            let now = Self::env().block_number();

            ButtonGame::reset::<ButtonGameEnvironment>(self, now)?;

            Self::emit_event(self.env(), Event::GameReset(GameReset { when: now }));
            Ok(())
        }

        #[ink(message)]
        fn deadline(&self) -> BlockNumber {
            ButtonGame::deadline(self)
        }

        #[ink(message)]
        fn access_control(&self) -> AccountId {
            ButtonGame::access_control(self)
        }

        #[ink(message)]
        fn last_presser(&self) -> Option<AccountId> {
            ButtonGame::last_presser(self)
        }

        #[ink(message)]
        fn reward_token(&self) -> AccountId {
            ButtonGame::reward_token(self)
        }

        #[ink(message)]
        fn ticket_token(&self) -> AccountId {
            ButtonGame::ticket_token(self)
        }

        #[ink(message)]
        fn balance(&self) -> ButtonResult<Balance> {
            let this = self.env().account_id();
            ButtonGame::balance::<ButtonGameEnvironment>(self, this)
        }

        #[ink(message)]
        fn set_access_control(&mut self, new_access_control: AccountId) -> ButtonResult<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            ButtonGame::set_access_control(self, new_access_control, caller, this)
        }

        #[ink(message)]
        fn terminate(&mut self) -> ButtonResult<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Owner(this);
            self.check_role(caller, required_role)?;
            self.env().terminate_contract(caller)
        }
    }

    impl EarlyBirdSpecial {
        #[ink(constructor)]
        pub fn new(
            ticket_token: AccountId,
            reward_token: AccountId,
            button_lifetime: BlockNumber,
        ) -> Self {
            let caller = Self::env().caller();
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Called new on a contract with no code hash");
            let required_role = Role::Initializer(code_hash);
            let access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);

            let role_check = <Self as AccessControlled>::check_role(
                access_control,
                caller,
                required_role,
                |why: InkEnvError| {
                    GameError::ContractCall(format!("Calling access control has failed: {:?}", why))
                },
                |role: Role| GameError::MissingRole(format!("{:?}", role)),
            );

            match role_check {
                Ok(_) => initialize_contract(|contract| {
                    Self::new_init(contract, ticket_token, reward_token, button_lifetime)
                }),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        fn new_init(
            &mut self,
            ticket_token: AccountId,
            reward_token: AccountId,
            button_lifetime: BlockNumber,
        ) {
            let now = Self::env().block_number();
            let deadline = now + button_lifetime;

            self.data.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            self.data.button_lifetime = button_lifetime;
            self.data.reward_token = reward_token;
            self.data.ticket_token = ticket_token;
            self.data.last_press = now;

            Self::emit_event(
                Self::env(),
                Event::ButtonCreated(ButtonCreated {
                    start: now,
                    deadline,
                    ticket_token,
                    reward_token,
                }),
            )
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<EarlyBirdSpecial>,
        {
            emitter.emit_event(event);
        }

        /// Returns own code hash
        #[ink(message)]
        pub fn code_hash(&self) -> ButtonResult<Hash> {
            self.env().own_code_hash().map_err(|why| {
                GameError::ContractCall(format!("Can't retrieve own code hash: {:?}", why))
            })
        }
    }
}
