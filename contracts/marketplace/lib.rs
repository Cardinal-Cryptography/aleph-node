#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

use ink_lang as ink;

#[ink::contract]
pub mod marketplace {
    use access_control::{
        traits::AccessControlled, Role, Role::Initializer, ACCESS_CONTROL_PUBKEY,
    };

    #[ink(storage)]
    pub struct Marketplace {
        price: Balance,
        at_block: BlockNumber,
        price_multiplier_numerator: Balance,
        price_multiplier_denominator: Balance,
        units: u32,
        sell_price_multiplier: Balance,
    }

    #[derive(Debug)]
    pub enum Error {
        AccessControlCall(ink_env::Error),
        MissingRole(Role),
    }

    impl AccessControlled for Marketplace {
        type ContractError = Error;
    }

    impl Marketplace {
        #[ink(constructor)]
        pub fn new(
            starting_price: Balance,
            price_multiplier_numerator: Balance,
            price_multiplier_denominator: Balance,
            units: u32,
            sell_price_multiplier: Balance,
        ) -> Self {
            match Self::ensure_role(Self::initializer()) {
                Err(error) => panic!("Failed to initialize the contract {:?}", error),
                Ok(_) => Marketplace {
                    price: starting_price,
                    at_block: Self::env().block_number(),
                    price_multiplier_numerator,
                    price_multiplier_denominator,
                    units,
                    sell_price_multiplier,
                },
            }
        }

        fn ensure_role(role: Role) -> Result<(), Error> {
            <Self as AccessControlled>::check_role(
                AccountId::from(ACCESS_CONTROL_PUBKEY),
                Self::env().caller(),
                role,
                |reason| Error::AccessControlCall(reason),
                |role| Error::MissingRole(role),
            )
        }

        fn initializer() -> Role {
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Failure to retrieve code hash.");
            Initializer(code_hash)
        }

        #[ink(message)]
        pub fn price(&self) -> Balance {
            self.price
        }

        #[ink(message)]
        pub fn at_block(&self) -> BlockNumber {
            self.at_block
        }

        #[ink(message)]
        pub fn block_multiplier(&self) -> (Balance, Balance) {
            (
                self.price_multiplier_numerator,
                self.price_multiplier_denominator,
            )
        }

        #[ink(message)]
        pub fn available_units(&self) -> u32 {
            self.units
        }

        #[ink(message)]
        pub fn buy(&mut self) {
            self.update_price();
            self.units = self.units.saturating_sub(1);
            self.price = self.price.saturating_mul(self.sell_price_multiplier);
        }

        fn update_price(&mut self) {
            let block = self.env().block_number();
            for _ in self.at_block..block {
                self.price = self
                    .price
                    .saturating_mul(self.price_multiplier_numerator)
                    .saturating_div(self.price_multiplier_denominator)
            }
            self.at_block = block;
        }
    }

    #[cfg(test)]
    mod tests {
        use ink_env::{block_number, test::advance_block, DefaultEnvironment};
        use ink_lang as ink;

        use super::*;

        type E = DefaultEnvironment;

        #[ink::test]
        fn initial_state() {
            let mut market = test_marketplace();

            assert_eq!(market.price(), 10000);
            assert_eq!(market.available_units(), 10)
        }

        #[ink::test]
        fn price_decreases_per_block() {
            let mut market = test_marketplace();

            advance_block::<E>();
            advance_block::<E>();

            assert_eq!(market.price(), 2500)
        }

        #[ink::test]
        fn buying_a_unit() {
            let mut market = test_marketplace();

            market.buy();

            assert_eq!(market.available_units(), 9);
            assert_eq!(market.price(), 50000);
        }

        fn test_marketplace() -> Marketplace {
            Marketplace {
                price: 10000,
                at_block: block_number::<E>(),
                price_multiplier_numerator: 1,
                price_multiplier_denominator: 2,
                units: 10,
                sell_price_multiplier: 5,
            }
        }
    }
}
