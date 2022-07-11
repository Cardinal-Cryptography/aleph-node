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
            todo!()
        }
    }

    // becasue ink! does not allow generics or trait default implementations
    impl IButtonGame for BlueButton {
        #[ink(message)]
        fn is_dead(&self) -> bool {
            ButtonGame::is_dead(self)
        }

        #[ink(message)]
        fn press(&mut self) -> Result<()> {
            ButtonGame::press(self)
        }

        #[ink(message)]
        fn deadline(&self) -> u32 {
            todo!()
        }

        #[ink(message)]
        fn score_of(&self, user: ink_env::AccountId) -> u32 {
            todo!()
        }

        #[ink(message)]
        fn can_play(&self, user: ink_env::AccountId) -> bool {
            todo!()
        }

        #[ink(message)]
        fn access_control(&self) -> ink_env::AccountId {
            todo!()
        }

        #[ink(message)]
        fn last_presser(&self) -> Option<ink_env::AccountId> {
            todo!()
        }

        #[ink(message)]
        fn get_button_token(&self) -> Result<ink_env::AccountId> {
            todo!()
        }

        #[ink(message)]
        fn get_balance(&self) -> Result<button::button::Balance> {
            todo!()
        }

        #[ink(message)]
        fn death(&mut self) -> Result<()> {
            todo!()
        }

        #[ink(message)]
        fn set_access_control(&mut self, access_control: ink_env::AccountId) -> Result<()> {
            todo!()
        }

        #[ink(message)]
        fn allow(&mut self, player: ink_env::AccountId) -> Result<()> {
            todo!()
        }

        #[ink(message)]
        fn bulk_allow(&mut self, players: Vec<ink_env::AccountId>) -> Result<()> {
            todo!()
        }

        #[ink(message)]
        fn disallow(&mut self, player: ink_env::AccountId) -> Result<()> {
            todo!()
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
            self.data.is_dead = false;
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
