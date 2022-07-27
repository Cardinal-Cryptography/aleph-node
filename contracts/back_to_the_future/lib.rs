#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// This is the BackToTheFuture
///
/// Larger rewards are distributed for postponing playing for as long as possible, but without letting TheButton die:
/// user_score = now - start
/// ThePressiah (the last player to click) still gets 50% of the tokens.

#[ink::contract(env = button::ButtonGameEnvironment)]
mod back_to_the_future {

    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use button::{
        ButtonData, ButtonGame, ButtonGameEnvironment, Error, IButtonGame, Result, Score,
    };
    use game_token::{BALANCE_OF_SELECTOR, TRANSFER_SELECTOR};
    use ink_env::Error as InkEnvError;
    use ink_lang::{
        codegen::{initialize_contract, EmitEvent},
        reflect::ContractEventBase,
    };
    use ink_prelude::{format, vec::Vec};
    use ink_storage::traits::SpreadAllocate;

    type Event = <BackToTheFuture as ContractEventBase>::Type;

    /// Event emitted when TheButton is created
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonCreated {
        #[ink(topic)]
        game_token: AccountId,
        start: BlockNumber,
        deadline: BlockNumber,
    }

    /// Event emitted when account is whitelisted to play the game
    #[ink(event)]
    #[derive(Debug)]
    pub struct AccountWhitelisted {
        #[ink(topic)]
        player: AccountId,
    }

    /// Event emitted when account is blacklisted from playing the game
    #[ink(event)]
    #[derive(Debug)]
    pub struct AccountBlacklisted {
        #[ink(topic)]
        player: AccountId,
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
    pub struct RewardClaimed {
        for_player: AccountId,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct BackToTheFuture {
        data: ButtonData,
    }

    impl AccessControlled for BackToTheFuture {
        type ContractError = Error;
    }

    impl ButtonGame for BackToTheFuture {
        fn get(&self) -> &ButtonData {
            &self.data
        }

        fn get_mut(&mut self) -> &mut ButtonData {
            &mut self.data
        }

        fn score(&self, now: BlockNumber) -> Score {
            now - self.get().last_press
        }
    }

    // becasue ink! does not allow generics or trait default implementations
    impl IButtonGame for BackToTheFuture {
        #[ink(message)]
        fn is_dead(&self) -> bool {
            let now = Self::env().block_number();
            ButtonGame::is_dead(self, now)
        }

        #[ink(message)]
        fn press(&mut self) -> Result<()> {
            let caller = self.env().caller();
            let now = Self::env().block_number();
            ButtonGame::press(self, now, caller)?;
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
        fn claim_reward(&mut self, for_player: AccountId) -> Result<()> {
            let this = self.env().account_id();
            let now = Self::env().block_number();

            ButtonGame::claim_reward::<ButtonGameEnvironment>(
                self,
                now,
                for_player,
                BALANCE_OF_SELECTOR,
                TRANSFER_SELECTOR,
                this,
            )?;

            Self::emit_event(
                self.env(),
                Event::RewardClaimed(RewardClaimed { for_player }),
            );
            Ok(())
        }

        #[ink(message)]
        fn deadline(&self) -> BlockNumber {
            ButtonGame::deadline(self)
        }

        #[ink(message)]
        fn score_of(&self, user: AccountId) -> Score {
            ButtonGame::score_of(self, user)
        }

        #[ink(message)]
        fn can_play(&self, user: AccountId) -> bool {
            ButtonGame::can_play(self, user)
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
        fn game_token(&self) -> AccountId {
            ButtonGame::game_token(self)
        }

        #[ink(message)]
        fn balance(&self) -> Result<Balance> {
            let this = self.env().account_id();
            ButtonGame::balance::<ButtonGameEnvironment>(self, BALANCE_OF_SELECTOR, this)
        }

        #[ink(message)]
        fn set_access_control(&mut self, new_access_control: AccountId) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            ButtonGame::set_access_control(self, new_access_control, caller, this)
        }

        #[ink(message)]
        fn allow(&mut self, player: AccountId) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            ButtonGame::allow(self, player, caller, this)?;
            Self::emit_event(
                self.env(),
                Event::AccountWhitelisted(AccountWhitelisted { player }),
            );
            Ok(())
        }

        #[ink(message)]
        fn bulk_allow(&mut self, players: Vec<AccountId>) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            ButtonGame::bulk_allow(self, players.clone(), caller, this)?;
            for player in players {
                Self::emit_event(
                    self.env(),
                    Event::AccountWhitelisted(AccountWhitelisted { player }),
                );
            }
            Ok(())
        }

        #[ink(message)]
        fn disallow(&mut self, player: AccountId) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            ButtonGame::disallow(self, player, caller, this)?;
            Self::emit_event(
                self.env(),
                Event::AccountBlacklisted(AccountBlacklisted { player }),
            );
            Ok(())
        }

        #[ink(message)]
        fn terminate(&mut self) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Owner(this);
            self.check_role(caller, required_role)?;
            self.env().terminate_contract(caller)
        }
    }

    impl BackToTheFuture {
        #[ink(constructor)]
        pub fn new(game_token: AccountId, button_lifetime: BlockNumber) -> Self {
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
                    Error::ContractCall(format!("Calling access control has failed: {:?}", why))
                },
                || Error::MissingRole,
            );

            match role_check {
                Ok(_) => initialize_contract(|contract| {
                    Self::new_init(contract, game_token, button_lifetime)
                }),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        fn new_init(&mut self, game_token: AccountId, button_lifetime: BlockNumber) {
            let now = Self::env().block_number();
            let deadline = now + button_lifetime;

            self.data.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            self.data.last_press = now;
            self.data.button_lifetime = button_lifetime;
            self.data.game_token = game_token;

            Self::emit_event(
                Self::env(),
                Event::ButtonCreated(ButtonCreated {
                    start: now,
                    deadline,
                    game_token,
                }),
            )
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<BackToTheFuture>,
        {
            emitter.emit_event(event);
        }

        /// Returns own code hash
        #[ink(message)]
        pub fn code_hash(&self) -> Result<Hash> {
            self.env().own_code_hash().map_err(|why| {
                Error::ContractCall(format!("Can't retrieve own code hash: {:?}", why))
            })
        }
    }
}
