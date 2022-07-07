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
    use button::button::{ButtonGame, Error, IButtonGame, Result};
    use button_token::{BALANCE_OF_SELECTOR, TRANSFER_SELECTOR};
    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        DefaultEnvironment, Error as InkEnvError,
    };
    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_prelude::{format, string::String, vec::Vec};
    use ink_storage::{traits::SpreadAllocate, Mapping};

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct BlueButton {}

    impl AccessControlled for BlueButton {
        type ContractError = Error;
    }

    // default concrete implementations
    impl ButtonGame for BlueButton {}

    // becasue ink! does not allow generics or trait default implementations
    impl IButtonGame for BlueButton {
        #[ink(message)]
        fn press(&mut self) -> Result<()> {
            ButtonGame::press(self)
        }
    }

    #[ink(impl)]
    impl BlueButton {
        #[ink(constructor)]
        pub fn new(button_token: AccountId, button_lifetime: u32) -> Self {
            todo!()
        }

        // #[ink(message)]
        // pub fn a(&mut self) -> Result<()> {
        //     todo!()
        // }
    }
}
