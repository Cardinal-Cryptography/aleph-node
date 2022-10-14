#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod snarcos {
    use snarcos_extension::StoreKeyError;

    #[ink(storage)]
    pub struct SnarcosExtension;

    #[ink(event)]
    pub struct KeyStored {}

    impl SnarcosExtension {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {}
        }

        #[ink(message)]
        pub fn trigger(&mut self) -> Result<(), StoreKeyError> {
            self.env().extension().store_key()?;
            self.env().emit_event(KeyStored {});
            Ok(())
        }
    }
}
