#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod access_control {

    use ink_lang::reflect::ContractEventBase;
    use ink_storage::{
        traits::{PackedLayout, SpreadAllocate, SpreadLayout, StorageLayout},
        Mapping,
    };
    use scale::{Decode, Encode};

    // #[derive(Encode, Decode, SpreadLayout, PackedLayout)]
    // pub type Roles = Mapping<Role, bool>;

    #[derive(Encode, Decode, Clone, Copy, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub enum Role {
        /// Indicates a superuser.
        Admin,
        /// Indicates account can terminate a contract.
        Owner,
        /// Indicates account can initialize a contract.
        Initializer,
    }

    // #[derive(Encode, Decode, SpreadLayout, PackedLayout, SpreadAllocate, Default)]
    // #[cfg_attr(
    //     feature = "std",
    //     derive(Debug, PartialEq, Eq, scale_info::TypeInfo, StorageLayout)
    // )]
    // pub struct Roles {
    //     /// Just store all.
    //     roles: Vec<u32>,
    // }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct AccessControl {
        /// only account with ownership can terminate the contract
        owner: AccountId,
        // Stores a mapping between user accounts and a list of their roles
        pub priviledges: Mapping<(AccountId, Role), ()>,
    }

    /// Event emitted when contract's owner is changed
    #[ink(event)]
    #[derive(Debug)]
    pub struct OwnershipTransferred {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Returned when an account which is not the owner calls a method with access control.
        NotOwner,
    }

    /// Result type    
    pub type Result<T> = core::result::Result<T, Error>;
    /// Event type
    pub type Event = <AccessControl as ContractEventBase>::Type;

    impl AccessControl {
        /// Creates a new contract.
        #[ink(constructor)]
        pub fn new() -> Self {
            // This call is required in order to correctly initialize the
            // `Mapping`s of our contract.
            ink_lang::utils::initialize_contract(|contract| Self::new_init(contract))
        }

        // TODO : caller is admin
        /// Initializes the contract.
        fn new_init(&mut self) {
            let caller = Self::env().caller();
            self.owner = caller;
        }

        // TODO : only admin
        #[ink(message)]
        pub fn grant_role(&mut self, account: AccountId, role: Role) -> Result<()> {
            self.priviledges.insert((account, role), &());
            Ok(())
        }

        // TODO : only admin
        #[ink(message)]
        pub fn revoke_role(&mut self, account: AccountId, role: Role) -> Result<()> {
            self.priviledges.remove((account, role));
            Ok(())
        }

        #[ink(message)]
        pub fn has_role(&self, account: AccountId, role: Role) -> bool {
            self.priviledges.get((account, role)).is_some()
        }

        /// Terminates the contract.
        ///
        /// can only be called by the contract owner
        #[ink(message)]
        pub fn terminate(&mut self) -> Result<()> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err(Error::NotOwner);
            }

            self.env().terminate_contract(caller)
        }

        /// Transfers ownership of the contract to a new account
        ///
        /// Can only be called by the current owner
        #[ink(message)]
        pub fn transfer_ownership(&mut self, to: AccountId) -> Result<()> {
            let caller = Self::env().caller();
            if caller != self.owner {
                return Err(Error::NotOwner);
            }
            self.owner = to;
            self.env()
                .emit_event(OwnershipTransferred { from: caller, to });
            Ok(())
        }

        /// Returns the contract owner.
        #[ink(message)]
        pub fn owner(&self) -> AccountId {
            self.owner
        }
    }
}
