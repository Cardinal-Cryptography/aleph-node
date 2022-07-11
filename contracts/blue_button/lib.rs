#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// This is the BlueButton
/// Larger rewards are distributed for postponing playing for as long as possible, but without letting TheButton die:
/// user_score = now - start
/// ThePressiah (the last player to click) still gets 50% of tokens
///
/// the game is played until TheButton dies

#[ink::contract]
mod blue_button {

    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use button::button::{ButtonData, ButtonGame, Error, IButtonGame, Result};
    use button_token::{BALANCE_OF_SELECTOR, TRANSFER_SELECTOR};
    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        DefaultEnvironment, Error as InkEnvError,
    };
    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_prelude::{format, string::String, vec::Vec};
    use ink_storage::{traits::SpreadAllocate, Mapping};

    /// Event type
    type Event = <BlueButton as ContractEventBase>::Type;

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

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct BlueButton {
        data: ButtonData,
    }

    impl AccessControlled for BlueButton {
        type ContractError = Error;
    }

    // default concrete implementations
    impl ButtonGame for BlueButton {
        fn get(&self) -> &ButtonData {
            &self.data
        }

        fn get_mut(&mut self) -> &mut ButtonData {
            &mut self.data
        }

        fn score(&self, now: u32) -> u32 {
            let deadline = ButtonGame::deadline(self);
            deadline - now
        }
    }

    // becasue ink! does not allow generics or trait default implementations
    impl IButtonGame for BlueButton {
        #[ink(message)]
        fn is_dead(&self) -> bool {
            let now = Self::env().block_number();
            ButtonGame::is_dead(self, now)
        }

        // TODO
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

        // TODO
        #[ink(message)]
        fn death(&mut self) -> Result<()> {
            todo!()
        }

        #[ink(message)]
        fn deadline(&self) -> u32 {
            ButtonGame::deadline(self)
        }

        #[ink(message)]
        fn score_of(&self, user: ink_env::AccountId) -> u32 {
            ButtonGame::score_of(self, user)
        }

        #[ink(message)]
        fn can_play(&self, user: ink_env::AccountId) -> bool {
            ButtonGame::can_play(self, user)
        }

        #[ink(message)]
        fn access_control(&self) -> ink_env::AccountId {
            ButtonGame::access_control(self)
        }

        #[ink(message)]
        fn last_presser(&self) -> Option<ink_env::AccountId> {
            ButtonGame::last_presser(self)
        }

        #[ink(message)]
        fn button_token(&self) -> Result<ink_env::AccountId> {
            ButtonGame::button_token(self)
        }

        #[ink(message)]
        fn balance(&self) -> Result<button::button::Balance> {
            let this = self.env().account_id();
            ButtonGame::balance(self, BALANCE_OF_SELECTOR, this)
        }

        #[ink(message)]
        fn set_access_control(&mut self, new_access_control: ink_env::AccountId) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            ButtonGame::set_access_control(self, new_access_control, caller, this)
        }

        #[ink(message)]
        fn allow(&mut self, player: ink_env::AccountId) -> Result<()> {
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
        fn bulk_allow(&mut self, players: Vec<ink_env::AccountId>) -> Result<()> {
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
        fn disallow(&mut self, player: ink_env::AccountId) -> Result<()> {
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

    impl BlueButton {
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
                Ok(_) => ink_lang::utils::initialize_contract(|contract| {
                    Self::new_init(contract, button_token, button_lifetime)
                }),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        fn new_init(&mut self, button_token: AccountId, button_lifetime: u32) {
            let now = Self::env().block_number();
            let deadline = now + button_lifetime;

            self.data.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            // self.data.is_dead = false;
            self.data.last_press = now;
            self.data.button_lifetime = button_lifetime;
            self.data.button_token = button_token;

            let event = Event::ButtonCreated(ButtonCreated {
                start: now,
                deadline,
                button_token,
            });

            Self::emit_event(Self::env(), event)
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<BlueButton>,
        {
            emitter.emit_event(event);
        }
    }
}
