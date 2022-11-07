#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod blender {
    #[ink(storage)]
    #[derive(Default)]
    pub struct Blender;

    impl Blender {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {}
        }

        #[ink(message)]
        pub fn nop(&self) {}
    }
}
