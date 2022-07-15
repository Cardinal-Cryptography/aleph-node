#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// This is the BackToTheFuture
///
/// Larger rewards are distributed for postponing playing for as long as possible, but without letting TheButton die:
/// user_score = now - start
/// ThePressiah (the last player to click) still gets 50% of the tokens.

#[ink::contract]
mod back_to_the_future {

    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use button::button::{ButtonData, ButtonGame, Error, IButtonGame, Result};
    use button_token::{BALANCE_OF_SELECTOR, TRANSFER_SELECTOR};
    use ink_env::{DefaultEnvironment, Error as InkEnvError};
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
        button_token: AccountId,
        start: u32,
        deadline: u32,
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
        when: u32,
    }

    /// Event emitted when button's death is triggered
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonDeath {}

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct BackToTheFuture {
        data: ButtonData,
    }

    impl AccessControlled for BackToTheFuture {
        type ContractError = Error;
    }

    // default concrete implementations
    impl ButtonGame for BackToTheFuture {
        fn get(&self) -> &ButtonData {
            &self.data
        }

        fn get_mut(&mut self) -> &mut ButtonData {
            &mut self.data
        }

        fn score(&self, now: u32) -> u32 {
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
        fn death(&self) -> Result<()> {
            let this = self.env().account_id();
            let now = Self::env().block_number();
            ButtonGame::death::<DefaultEnvironment>(
                self,
                now,
                BALANCE_OF_SELECTOR,
                TRANSFER_SELECTOR,
                this,
            )?;
            Self::emit_event(self.env(), Event::ButtonDeath(ButtonDeath {}));
            Ok(())
        }

        #[ink(message)]
        fn deadline(&self) -> u32 {
            ButtonGame::deadline(self)
        }

        #[ink(message)]
        fn score_of(&self, user: AccountId) -> u32 {
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
        fn button_token(&self) -> Result<AccountId> {
            ButtonGame::button_token(self)
        }

        #[ink(message)]
        fn balance(&self) -> Result<Balance> {
            let this = self.env().account_id();
            ButtonGame::balance::<DefaultEnvironment>(self, BALANCE_OF_SELECTOR, this)
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
        pub fn new(button_token: AccountId, button_lifetime: u32) -> Self {
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
                    Self::new_init(contract, button_token, button_lifetime)
                }),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        fn new_init(&mut self, button_token: AccountId, button_lifetime: u32) {
            let now = Self::env().block_number();
            let deadline = now + button_lifetime;

            self.data.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            self.data.last_press = now;
            self.data.button_lifetime = button_lifetime;
            self.data.button_token = button_token;

            Self::emit_event(
                Self::env(),
                Event::ButtonCreated(ButtonCreated {
                    start: now,
                    deadline,
                    button_token,
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

    // NOTE: can't test because off-chain environment does not support `own_code_hash`
    /*
    #[cfg(test)]
    mod tests {
        use super::*;
        use button_token::{ButtonToken, Event as ButtonTokenEvent};
        use ink_lang as ink;

        #[ink::test]
        fn play_the_game() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();

            let alice = accounts.alice;
            let bob = accounts.bob;
            let charlie = accounts.charlie;

            let button_token_address = accounts.frank; //AccountId::from([0xFA; 32]);
            let game_address = accounts.django; //AccountId::from([0xF9; 32]);

            // alice deploys the token contract
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(alice);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(button_token_address);
            let mut button_token = ButtonToken::new(1000);

            // alice deploys the game contract
            let button_lifetime = 3;
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(alice);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(game_address);
            let mut game = RedButton::new(button_token_address, button_lifetime);

            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            let button_created_event = &emitted_events[1];
            let decoded_event: Event =
                <Event as scale::Decode>::decode(&mut &button_created_event.data[..])
                    .expect("Can't decode as Event");

            match decoded_event {
                Event::ButtonCreated(ButtonCreated {
                    start,
                    deadline,
                    button_token,
                }) => {
                    assert_eq!(deadline, button_lifetime, "Wrong ButtonCreated.deadline");
                    assert_eq!(start, 0, "Wrong ButtonCreated.start");
                    assert_eq!(
                        button_token, button_token_address,
                        "Wrong ButtonCreated.button_token"
                    );
                }
                _ => panic!("Wrong event emitted"),
            }

            // Alice transfer all token balance to the game
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(alice);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(button_token_address);

            assert!(
                button_token.transfer(game_address, 999).is_ok(),
                "Transfer call failed"
            );

            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            let transfer_event = &emitted_events[2];
            let decoded_event: ButtonTokenEvent =
                <ButtonTokenEvent as scale::Decode>::decode(&mut &transfer_event.data[..])
                    .expect("Can't decode as Event");

            match decoded_event {
                ButtonTokenEvent::Transfer(event) => {
                    assert_eq!(event.value, 999, "Wrong Transfer.value");
                    assert_eq!(event.from, Some(alice), "Wrong Transfer.from");
                    assert_eq!(event.to, game_address, "Wrong Transfer.from");
                }
                _ => panic!("Wrong event emitted"),
            }

            // Alice is the owner and whitelists accounts for playing
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(alice);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(game_address);
            assert!(
                game.bulk_allow(vec![bob, charlie]).is_ok(),
                "Bulk allow call failed"
            );

            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(bob);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(game_address);
            assert!(game.press().is_ok(), "Press call failed");

            ink_env::test::advance_block::<ink_env::DefaultEnvironment>();

            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(charlie);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(game_address);
            assert!(game.press().is_ok(), "Press call failed");

            // NOTE : we cannot test reward distribution, cross-contract calls are not yet supported in the test environment
        }
    }*/
}
