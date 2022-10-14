#![cfg_attr(not(feature = "std"), no_std)]

use ink_env::Environment;
use ink_lang as ink;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CustomEnvironment {}

// All default, but `ChainExtension`.
impl Environment for CustomEnvironment {
    const MAX_EVENT_TOPICS: usize = <ink_env::DefaultEnvironment as Environment>::MAX_EVENT_TOPICS;

    type AccountId = <ink_env::DefaultEnvironment as Environment>::AccountId;
    type Balance = <ink_env::DefaultEnvironment as Environment>::Balance;
    type Hash = <ink_env::DefaultEnvironment as Environment>::Hash;
    type Timestamp = <ink_env::DefaultEnvironment as Environment>::Timestamp;
    type BlockNumber = <ink_env::DefaultEnvironment as Environment>::BlockNumber;

    type ChainExtension = snarcos_extension::StoreKeyExtension;
}

#[ink::contract(env = crate::CustomEnvironment)]
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
