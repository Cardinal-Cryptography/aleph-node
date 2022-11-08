#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

#[openbrush::contract]
pub mod my_psp22 {
    use ink_storage::traits::SpreadAllocate;
    use openbrush::{contracts::psp22::*, traits::Storage};

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, Storage)]
    pub struct Contract {
        #[storage_field]
        psp22: Data,
    }

    impl PSP22 for Contract {}

    impl Contract {
        #[ink(constructor)]
        pub fn new(total_supply: Balance) -> Self {
            ink_lang::codegen::initialize_contract(|instance: &mut Contract| {
                instance
                    ._mint_to(instance.env().caller(), total_supply)
                    .expect("Should mint");
            })
        }
    }
}
