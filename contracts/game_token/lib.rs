#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

pub use crate::game_token::{
    ALLOWANCE_SELECTOR, BALANCE_OF_SELECTOR, TOTAL_SUPPLY_SELECTOR, TRANSFER_SELECTOR,
};

#[openbrush::contract]
pub mod game_token {
    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use ink_env::Error as InkEnvError;
    use ink_prelude::{format, string::String};
    use ink_storage::traits::SpreadAllocate;
    use openbrush::{contracts::psp22::*, traits::Storage};

    pub const TOTAL_SUPPLY_SELECTOR: [u8; 4] = [0x16, 0x2d, 0xf8, 0xc2];
    pub const BALANCE_OF_SELECTOR: [u8; 4] = [0x65, 0x68, 0x38, 0x2f];
    pub const ALLOWANCE_SELECTOR: [u8; 4] = [0x4d, 0x47, 0xd9, 0x21];
    pub const TRANSFER_SELECTOR: [u8; 4] = [0xdb, 0x20, 0xf9, 0xf5];

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, Storage)]
    pub struct GameToken {
        #[storage_field]
        psp22: psp22::Data,
        /// access control contract
        access_control: AccountId,
    }

    impl PSP22 for GameToken {}

    impl AccessControlled for GameToken {
        type ContractError = PSP22Error;
    }

    /// Result type
    pub type Result<T> = core::result::Result<T, PSP22Error>;

    impl GameToken {
        /// Creates a new contract with the specified initial supply.
        ///
        /// Will revert if called from an account without a proper role        
        #[ink(constructor)]
        pub fn new(total_supply: Balance) -> Self {
            let caller = Self::env().caller();
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Called new on a contract with no code hash");
            let required_role = Role::Initializer(code_hash);

            let role_check = <Self as AccessControlled>::check_role(
                AccountId::from(ACCESS_CONTROL_PUBKEY),
                caller,
                required_role,
                |why: InkEnvError| {
                    PSP22Error::Custom(format!("Calling access control has failed: {:?}", why))
                },
                || PSP22Error::Custom(String::from("MissingRole")),
            );

            match role_check {
                Ok(_) => ink_lang::codegen::initialize_contract(|instance: &mut GameToken| {
                    instance
                        ._mint(instance.env().caller(), total_supply)
                        .expect("Should mint");

                    instance.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
                }),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        /// Terminates the contract.
        ///
        /// can only be called by the contract's Owner
        #[ink(message, selector = 7)]
        pub fn terminate(&mut self) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Owner(this);

            self.check_role(caller, required_role)?;
            self.env().terminate_contract(caller)
        }

        /// Returns the contract's access control contract address
        #[ink(message, selector = 8)]
        pub fn access_control(&self) -> AccountId {
            self.access_control
        }

        fn check_role(&self, account: AccountId, role: Role) -> Result<()> {
            <Self as AccessControlled>::check_role(
                self.access_control,
                account,
                role,
                |why: InkEnvError| {
                    PSP22Error::Custom(format!("Calling access control has failed: {:?}", why))
                },
                || PSP22Error::Custom(String::from("MissingRole")),
            )
        }

        /// Sets new access control contract address
        ///
        /// Can only be called by the contract's Owner
        #[ink(message, selector = 9)]
        pub fn set_access_control(&mut self, access_control: AccountId) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Owner(this);

            self.check_role(caller, required_role)?;
            self.access_control = access_control;
            Ok(())
        }

        /// Returns own code hash
        #[ink(message, selector = 10)]
        pub fn code_hash(&self) -> Result<Hash> {
            Self::env()
                .own_code_hash()
                .map_err(|why| PSP22Error::Custom(format!("Calling control has failed: {:?}", why)))
        }
    }
}
