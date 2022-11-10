#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]
#![allow(clippy::let_unit_value)]

pub use crate::game_token::{
    ALLOWANCE_SELECTOR, BALANCE_OF_SELECTOR, BURN_SELECTOR, MINT_SELECTOR, TRANSFER_FROM_SELECTOR,
    TRANSFER_SELECTOR,
};

#[openbrush::contract]
pub mod game_token {
    use access_control::{roles::Role, traits::AccessControlled, ACCESS_CONTROL_PUBKEY};
    use ink_env::Error as InkEnvError;
    use ink_lang::{
        codegen::{EmitEvent, Env},
        reflect::ContractEventBase,
    };
    use ink_prelude::{format, string::String};
    use ink_storage::traits::SpreadAllocate;
    use openbrush::{
        contracts::psp22::{
            extensions::{burnable::*, metadata::*, mintable::*},
            Internal,
        },
        traits::Storage,
    };

    pub const BALANCE_OF_SELECTOR: [u8; 4] = [0x65, 0x68, 0x38, 0x2f];
    pub const TRANSFER_SELECTOR: [u8; 4] = [0xdb, 0x20, 0xf9, 0xf5];
    pub const TRANSFER_FROM_SELECTOR: [u8; 4] = [0x54, 0xb3, 0xc7, 0x6e];
    pub const ALLOWANCE_SELECTOR: [u8; 4] = [0x4d, 0x47, 0xd9, 0x21];
    pub const MINT_SELECTOR: [u8; 4] = [0xfc, 0x3c, 0x75, 0xd4];
    pub const BURN_SELECTOR: [u8; 4] = [0x7a, 0x9d, 0xa5, 0x10];

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, Storage)]
    pub struct GameToken {
        #[storage_field]
        psp22: psp22::Data,
        #[storage_field]
        metadata: metadata::Data,
        /// access control contract
        access_control: AccountId,
    }

    impl PSP22 for GameToken {}

    impl PSP22Metadata for GameToken {}

    impl PSP22Mintable for GameToken {
        #[ink(message)]
        fn mint(&mut self, account: AccountId, amount: Balance) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Minter(this);

            self.check_role(caller, required_role)?;
            self._mint_to(account, amount)
        }
    }

    impl PSP22Burnable for GameToken {
        #[ink(message)]
        fn burn(&mut self, account: AccountId, amount: Balance) -> Result<()> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            let required_role = Role::Burner(this);

            self.check_role(caller, required_role)?;
            self._burn_from(account, amount)
        }
    }

    // emit events
    // https://github.com/w3f/PSPs/blob/master/PSPs/psp-22.md
    impl Internal for GameToken {
        fn _emit_transfer_event(
            &self,
            _from: Option<AccountId>,
            _to: Option<AccountId>,
            _amount: Balance,
        ) {
            GameToken::emit_event(
                self.env(),
                Event::Transfer(Transfer {
                    from: _from,
                    to: _to,
                    value: _amount,
                }),
            );
        }

        fn _emit_approval_event(&self, _owner: AccountId, _spender: AccountId, _amount: Balance) {
            GameToken::emit_event(
                self.env(),
                Event::Approval(Approval {
                    owner: _owner,
                    spender: _spender,
                    value: _amount,
                }),
            );
        }
    }

    impl AccessControlled for GameToken {
        type ContractError = PSP22Error;
    }

    /// Result type
    pub type Result<T> = core::result::Result<T, PSP22Error>;
    /// Event type
    pub type Event = <GameToken as ContractEventBase>::Type;

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

    impl GameToken {
        /// Creates a new game token with the specified initial supply.
        ///
        /// The token will have its name and symbol set in metadata to the specified values.
        /// Decimals are fixed at 18.
        ///
        /// Will revert if called from an account without a proper role
        #[ink(constructor)]
        pub fn new(name: String, symbol: String) -> Self {
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
                    PSP22Error::Custom(
                        format!("Calling access control has failed: {:?}", why).into(),
                    )
                },
                |role: Role| PSP22Error::Custom(format!("MissingRole:{:?}", role).into()),
            );

            match role_check {
                Ok(_) => ink_lang::codegen::initialize_contract(|instance: &mut GameToken| {
                    instance.metadata.name = Some(name.into());
                    instance.metadata.symbol = Some(symbol.into());
                    instance.metadata.decimals = 12;
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
                    PSP22Error::Custom(
                        format!("Calling access control has failed: {:?}", why).into(),
                    )
                },
                |role: Role| PSP22Error::Custom(format!("MissingRole:{:?}", role).into()),
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
            Self::env().own_code_hash().map_err(|why| {
                PSP22Error::Custom(format!("Can't retrieve own code hash: {:?}", why).into())
            })
        }
    }
}
