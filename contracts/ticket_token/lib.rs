#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

use ink_env::{DefaultEnvironment, Environment};

pub use crate::ticket_token::{BALANCE_OF_SELECTOR, TRANSFER_SELECTOR};

pub enum TicketEnvironment {}

impl Environment for TicketEnvironment {
    const MAX_EVENT_TOPICS: usize = <DefaultEnvironment as Environment>::MAX_EVENT_TOPICS;

    type AccountId = <DefaultEnvironment as Environment>::AccountId;
    type Balance = u64;
    type Hash = <DefaultEnvironment as Environment>::Hash;
    type BlockNumber = u64;
    type Timestamp = <DefaultEnvironment as Environment>::Timestamp;
    type ChainExtension = <DefaultEnvironment as Environment>::ChainExtension;
}

#[openbrush::contract]
pub mod ticket_token {
    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use ink_env::Error as InkEnvError;
    use ink_lang::{
        codegen::{EmitEvent, Env},
        reflect::ContractEventBase,
    };
    use ink_prelude::format;
    use ink_storage::traits::SpreadAllocate;
    use openbrush::{contracts::psp22::*, traits::Storage};

    pub const BALANCE_OF_SELECTOR: [u8; 4] = [0x65, 0x68, 0x38, 0x2f];
    pub const TRANSFER_SELECTOR: [u8; 4] = [0xdb, 0x20, 0xf9, 0xf5];

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, Storage)]
    pub struct TicketToken {
        #[storage_field]
        psp22: psp22::Data,
        /// access control contract
        access_control: AccountId,
    }

    impl PSP22 for TicketToken {}

    // emit events
    // https://github.com/w3f/PSPs/blob/master/PSPs/psp-22.md
    impl Internal for TicketToken {
        fn _emit_transfer_event(
            &self,
            _from: Option<AccountId>,
            _to: Option<AccountId>,
            _amount: Balance,
        ) {
            TicketToken::emit_event(
                self.env(),
                Event::Transfer(Transfer {
                    from: _from,
                    to: _to,
                    value: _amount,
                }),
            );
        }

        fn _emit_approval_event(&self, _owner: AccountId, _spender: AccountId, _amount: Balance) {
            TicketToken::emit_event(
                self.env(),
                Event::Approval(Approval {
                    owner: _owner,
                    spender: _spender,
                    value: _amount,
                }),
            );
        }
    }

    impl AccessControlled for TicketToken {
        type ContractError = PSP22Error;
    }

    /// Result type
    pub type Result<T> = core::result::Result<T, PSP22Error>;
    /// Event type
    pub type Event = <TicketToken as ContractEventBase>::Type;

    /// Event emitted when a token transfer occurs.
    #[ink(event)]
    #[derive(Debug)]
    pub struct Transfer {
        #[ink(topic)]
        pub from: Option<AccountId>,
        #[ink(topic)]
        pub to: Option<AccountId>,
        pub value: Balance,
    }

    /// Event emitted when an approval occurs that `spender` is allowed to withdraw
    /// up to the amount of `value` tokens from `owner`.
    #[ink(event)]
    #[derive(Debug)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        value: Balance,
    }

    impl TicketToken {
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
                |role: Role| PSP22Error::Custom(format!("MissingRole:{:?}", role)),
            );

            match role_check {
                Ok(_) => ink_lang::codegen::initialize_contract(|instance: &mut TicketToken| {
                    instance
                        ._mint(instance.env().caller(), total_supply)
                        .expect("Should mint");

                    instance.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
                }),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        pub fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event);
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
                |role: Role| PSP22Error::Custom(format!("MissingRole:{:?}", role)),
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
