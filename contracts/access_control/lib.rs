#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::access_control::{Role, HAS_ROLE_SELECTOR};
use ink_lang as ink;

#[ink::contract]
mod access_control {

    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_storage::{
        traits::{PackedLayout, SpreadAllocate, SpreadLayout},
        Mapping,
    };
    use scale::{Decode, Encode};

    pub const HAS_ROLE_SELECTOR: [u8; 4] = [0, 0, 0, 3];

    #[derive(Debug, Encode, Decode, Clone, Copy, SpreadLayout, PackedLayout, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    pub enum Role {
        /// Indicates a superuser.
        Admin(AccountId),
        /// Indicates account can terminate a contract.
        Owner(AccountId),
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
        /// caller is granted admin and owner piviledges
        fn new_init(&mut self) {
            let caller = Self::env().caller();
            let this = self.env().account_id();
            self.priviledges.insert((caller, Role::Admin(this)), &());
            self.priviledges.insert((caller, Role::Owner(this)), &());
        }

        // TODO : no-op if role exists?
        #[ink(message, selector = 1)]
        /// gives a role to an account
        ///
        /// Can only be called by an admin role on this contract                
        pub fn grant_role(&mut self, account: AccountId, role: Role) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            self.check_role(caller, Role::Admin(this))?;
            self.priviledges.insert((account, role), &());

            let event = Event::RoleGranted(RoleGranted {
                by: caller,
                to: account,
                role,
            });
            Self::emit_event(self.env(), event);

            Ok(())
        }

        #[ink(message, selector = 2)]
        /// revokes a role from an account
        ///
        /// Can only be called by an admin role on this contract        
        pub fn revoke_role(&mut self, account: AccountId, role: Role) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            self.check_role(caller, Role::Admin(this))?;
            self.priviledges.remove((account, role));

            let event = Event::RoleRevoked(RoleRevoked {
                by: caller,
                from: account,
                role,
            });
            Self::emit_event(self.env(), event);

            Ok(())
        }

        #[ink(message, selector = 3)]
        /// returns true if account has a role
        pub fn has_role(&self, account: AccountId, role: Role) -> bool {
            self.priviledges.get((account, role)).is_some()
        }

        /// Terminates the contract.
        ///
        /// can only be called by the contract owner
        #[ink(message, selector = 4)]
        pub fn terminate(&mut self) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            self.check_role(caller, Role::Owner(this))?;
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink_lang as ink;

        #[ink::test]
        fn access_control() {
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();

            let alice = accounts.alice;
            let bob = accounts.alice;
            let charlie = accounts.charlie;

            let contract_address = accounts.django; //AccountId::from([0xF9; 32]);

            // alice deploys the access control contract
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(alice);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(contract_address);
            let mut access_control = AccessControl::new();

            // alice should be admin
            assert!(
                access_control.has_role(alice, Role::Admin(contract_address)),
                "deployer is not admin"
            );

            // alice should be owner
            assert!(
                access_control.has_role(alice, Role::Owner(contract_address)),
                "deployer is not admin"
            );

            // alice grants admin rights to bob
            assert!(
                access_control
                    .grant_role(bob, Role::Admin(contract_address))
                    .is_ok(),
                "Alice's grant_role call failed"
            );

            assert!(
                access_control.has_role(bob, Role::Admin(contract_address)),
                "Bob is not admin"
            );

            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(bob);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(contract_address);

            // bob grants admin rights to charlier
            assert!(
                access_control
                    .grant_role(charlie, Role::Admin(contract_address))
                    .is_ok(),
                "Bob's grant_role by call failed"
            );

            assert!(
                access_control.has_role(charlie, Role::Admin(contract_address)),
                "Charlie is not admin"
            );
        }
    }
}
