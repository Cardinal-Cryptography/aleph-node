#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod access_control {

    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_storage::{
        traits::{PackedLayout, SpreadAllocate, SpreadLayout},
        Mapping,
    };
    use scale::{Decode, Encode};

    #[derive(Debug, Encode, Decode, Clone, Copy, SpreadLayout, PackedLayout)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    pub enum Role {
        /// Indicates a superuser.
        Admin,
        /// Indicates account can terminate a contract.
        Owner,
        /// Indicates account can initialize a contract from a given code hash.
        Initializer(Hash),
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct AccessControl {
        /// Stores a de-facto mapping between user accounts and a list of their roles
        pub priviledges: Mapping<(AccountId, Role), ()>,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct RoleGranted {
        #[ink(topic)]
        by: AccountId,
        #[ink(topic)]
        to: AccountId,
        #[ink(topic)]
        role: Role,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct RoleRevoked {
        #[ink(topic)]
        by: AccountId,
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        role: Role,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum AccessControlError {
        MissingRole,
    }

    /// Result type    
    pub type Result<T> = core::result::Result<T, AccessControlError>;
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

        /// Initializes the contract.
        ///
        /// caller is granted admin piviledges
        fn new_init(&mut self) {
            let caller = Self::env().caller();
            self.priviledges.insert((caller, Role::Admin), &());
        }

        // TODO : no-op if role exists?
        #[ink(message)]
        pub fn grant_role(&mut self, account: AccountId, role: Role) -> Result<()> {
            let caller = self.env().caller();
            self.check_role(caller, Role::Admin)?;
            self.priviledges.insert((account, role), &());

            let event = Event::RoleGranted(RoleGranted {
                by: caller,
                to: account,
                role,
            });
            Self::emit_event(self.env(), event);

            Ok(())
        }

        #[ink(message)]
        pub fn revoke_role(&mut self, account: AccountId, role: Role) -> Result<()> {
            let caller = self.env().caller();
            self.check_role(caller, Role::Admin)?;
            self.priviledges.remove((account, role));

            let event = Event::RoleRevoked(RoleRevoked {
                by: caller,
                from: account,
                role,
            });
            Self::emit_event(self.env(), event);

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
            self.check_role(caller, Role::Admin)?;
            self.env().terminate_contract(caller)
        }

        fn check_role(&self, account: AccountId, role: Role) -> Result<()> {
            if !self.has_role(account, role) {
                return Err(AccessControlError::MissingRole);
            }
            Ok(())
        }

        fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event);
        }
    }
}
