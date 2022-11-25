//! Implement a Dutch auction of one token for another.
//! 
//! This is almost a clone of Marketplace - purpose of this contract is
//! to test Marketplace's upgradability capabilities.

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]
#![allow(clippy::let_unit_value)]

use ink_lang as ink;

pub const RESET_SELECTOR: [u8; 4] = [0x00, 0x00, 0x00, 0x01];

#[ink::contract]
pub mod marketplace_v2 {
    use access_control::{roles::Role, traits::AccessControlled, ACCESS_CONTROL_PUBKEY};
    use game_token::BURN_SELECTOR as REWARD_BURN_SELECTOR;
    use ink_env::{
        call::{build_call, Call, DelegateCall, ExecutionInput, Selector},
        CallFlags,
    };
    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_prelude::{format, string::String};
    use ink_env::{set_code_hash, clear_contract_storage, contract_storage_contains};
    use ink_storage::traits::SpreadAllocate;
    use openbrush::contracts::psp22::PSP22Error;
    use ticket_token::{
        BALANCE_OF_SELECTOR as TICKET_BALANCE_SELECTOR,
        TRANSFER_SELECTOR as TRANSFER_TICKET_SELECTOR,
    };

    type Event = <Marketplace as ContractEventBase>::Type;
    type SelectorData = [u8; 4];

    const DUMMY_DATA: &[u8] = &[0x0];

    //const OLD_STORAGE_KEY: u32 = openbrush::storage_unique_key!(MarketplaceDataV1);
    const OLD_STORAGE_KEY: u32 = 101;
    #[derive(Default, Debug)]
    #[openbrush::upgradeable_storage(OLD_STORAGE_KEY)]
    pub struct MarketplaceDataV1 {
        total_proceeds: Balance,
        tickets_sold: Balance,
        min_price: Balance,
        current_start_block: BlockNumber,
        auction_length: BlockNumber,
        sale_multiplier: Balance,
        ticket_token: AccountId,
        reward_token: AccountId,
    }

    // Storage struct with different order of fields - for upgrade testing 
    // Also there is new field (migration_performed) added
    const STORAGE_KEY: u32 = 201;
    #[derive(Default, Debug)]
    #[openbrush::upgradeable_storage(STORAGE_KEY)]
    pub struct MarketplaceDataV2 {
        migration_performed: bool,
        sale_multiplier: Balance,
        min_price: Balance,
        current_start_block: BlockNumber,
        tickets_sold: Balance,
        auction_length: BlockNumber,
        reward_token: AccountId,
        ticket_token: AccountId,
        total_proceeds: Balance,
        new_field: AccountId,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Marketplace {
        _old_data: MarketplaceDataV1,
        data: MarketplaceDataV2,
    }

    #[derive(Eq, PartialEq, Debug, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        MissingRole(Role),
        ContractCall(String),
        PSP22TokenCall(PSP22Error),
        MaxPriceExceeded,
        MarketplaceEmpty,
        UpgradeFailed,
        MigrationAlreadyPerformed,
    }

    #[ink(event)]
    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Bought {
        #[ink(topic)]
        pub account_id: AccountId,
        pub price: Balance,
    }

    #[ink(event)]
    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Reset;

    impl From<ink_env::Error> for Error {
        fn from(inner: ink_env::Error) -> Self {
            Error::ContractCall(format!("{:?}", inner))
        }
    }

    impl From<PSP22Error> for Error {
        fn from(inner: PSP22Error) -> Self {
            Error::PSP22TokenCall(inner)
        }
    }

    impl AccessControlled for Marketplace {
        type ContractError = Error;
    }

    impl Marketplace {
        /// This should never be called, as only code hash of this contract is required
        /// to perform an upgrade
        #[ink(constructor)]
        pub fn new() -> Self {
            Self::ensure_role(Self::initializer())
                .unwrap_or_else(|e| panic!("Failed to initialize the contract {:?}", e));

            Marketplace {
                _old_data: Default::default(),
                data: Default::default(),
            }
        }

        /// The length of each auction of a single ticket in blocks.
        ///
        /// The contract will decrease the price linearly from `average_price() * sale_multiplier()`
        /// to `min_price()` over this period. The auction doesn't end after the period elapses -
        /// the ticket remains available for purchase at `min_price()`.
        #[ink(message)]
        pub fn auction_length(&self) -> BlockNumber {
            self.data.auction_length
        }

        /// The block at which the auction of the current ticket started.
        #[ink(message)]
        pub fn current_start_block(&self) -> BlockNumber {
            self.data.current_start_block
        }

        /// The price the contract would charge when buying at the current block.
        #[ink(message)]
        pub fn price(&self) -> Balance {
            self.current_price()
        }

        /// The average price over all sales the contract made.
        #[ink(message)]
        pub fn average_price(&self) -> Balance {
            self.data.total_proceeds.saturating_div(self.data.tickets_sold)
        }

        /// The multiplier applied to the average price after each sale.
        ///
        /// The contract tracks the average price of all sold tickets and starts off each new
        /// auction at `price() = average_price() * sale_multiplier()`.
        #[ink(message)]
        pub fn sale_multiplier(&self) -> Balance {
            self.data.sale_multiplier
        }

        /// Number of tickets available for sale.
        ///
        /// The tickets will be auctioned off one by one.
        #[ink(message)]
        pub fn available_tickets(&self) -> Result<Balance, Error> {
            self.ticket_balance()
        }

        /// The minimal price the contract allows.
        #[ink(message)]
        pub fn min_price(&self) -> Balance {
            self.data.min_price
        }

        /// Update the minimal price.
        #[ink(message)]
        pub fn set_min_price(&mut self, value: Balance) -> Result<(), Error> {
            Self::ensure_role(self.admin())?;

            self.data.min_price = value;

            Ok(())
        }

        /// Address of the reward token contract this contract will accept as payment.
        #[ink(message)]
        pub fn reward_token(&self) -> AccountId {
            self.data.reward_token
        }

        /// Address of the ticket token contract this contract will auction off.
        #[ink(message)]
        pub fn ticket_token(&self) -> AccountId {
            self.data.ticket_token
        }

        /// Buy one ticket at the current_price.
        ///
        /// The caller should make an approval for at least `price()` reward tokens to make sure the
        /// call will succeed. The caller can specify a `max_price` - the call will fail if the
        /// current price is greater than that.
        #[ink(message)]
        pub fn buy(&mut self, max_price: Option<Balance>) -> Result<(), Error> {
            if self.ticket_balance()? == 0 {
                return Err(Error::MarketplaceEmpty);
            }

            let price = self.current_price();
            if let Some(max_price) = max_price {
                if price > max_price {
                    return Err(Error::MaxPriceExceeded);
                }
            }

            let account_id = self.env().caller();

            self.take_payment(account_id, price)?;
            self.give_ticket(account_id)?;

            self.data.total_proceeds = self.data.total_proceeds.saturating_add(price);
            self.data.tickets_sold = self.data.tickets_sold.saturating_add(1);
            self.data.current_start_block = self.env().block_number();
            Self::emit_event(self.env(), Event::Bought(Bought { price, account_id }));

            Ok(())
        }

        /// Re-start the auction from the current block.
        ///
        /// Note that this will keep the average estimate from previous auctions.
        ///
        /// Requires `Role::Admin`.
        #[ink(message, selector = 0x00000001)]
        pub fn reset(&mut self) -> Result<(), Error> {
            Self::ensure_role(self.admin())?;

            self.data.current_start_block = self.env().block_number();
            Self::emit_event(self.env(), Event::Reset(Reset {}));

            Ok(())
        }

        /// Terminates the contract
        ///
        /// Should only be called by the contract Owner
        #[ink(message)]
        pub fn terminate(&mut self) -> Result<(), Error> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            Self::ensure_role(Role::Owner(this))?;
            self.env().terminate_contract(caller)
        }

        /// Sets new code hash, updates contract code
        /// 
        /// Option: you can pass a selector of the function that
        /// performs a migration to the new storage struct.
        /// This allows for 'atomic' upgrade + migration
        #[ink(message)]
        pub fn set_code(&mut self, code_hash: [u8; 32], migration_selector: Option<SelectorData>) -> Result<(), Error> {
            let this = self.env().account_id();
            Self::ensure_role(Role::Owner(this))?;

            if set_code_hash(&code_hash).is_err() {
                return Err(Error::UpgradeFailed);
            };

            if let Some(selector_bytes) = migration_selector {
                let selector = Selector::from(selector_bytes);
                let code_hash = Hash::from(code_hash);
                build_call::<Environment>()
                    .call_type(DelegateCall::new().code_hash(code_hash))
                    .exec_input(ExecutionInput::new(selector))
                    .call_flags(CallFlags::default().set_tail_call(true))
                    // placeholder, it's a tail call anyway
                    .returns::<()>()
                    .fire()?;
            }

            Ok(())
        }

        /// Performs a migration from old contract code
        #[ink(message)]
        pub fn migrate(&mut self) -> Result<(), Error> {
            // This should work - if migration was not performed, then
            // `migration_performed` will be initialised to default, so to `false`
            if self.data.migration_performed {
                return Err(Error::MigrationAlreadyPerformed);
            }

            // Only owner can perform migration (?)
            let this = self.env().account_id();
            Self::ensure_role(Role::Owner(this))?;

            // Of course, this migration looks weird, as the upgrade itself is
            // pretty meaningless
            self.data.sale_multiplier = self._old_data.sale_multiplier;
            self.data.min_price = self._old_data.min_price;
            self.data.current_start_block = self._old_data.current_start_block;
            self.data.tickets_sold = self._old_data.tickets_sold;
            self.data.auction_length = self._old_data.auction_length;
            self.data.reward_token = self._old_data.reward_token;
            self.data.ticket_token = self._old_data.ticket_token;
            self.data.total_proceeds = self._old_data.total_proceeds;

            // Change key format
            let key_bytes_pref = OLD_STORAGE_KEY.to_le_bytes();
            let mut key_bytes: [u8; 32] = [0; 32];
            for i in 0..4 {
                key_bytes[i] = key_bytes_pref[i];
            }

            // Assert that there is something to clear
            assert!(matches!(contract_storage_contains(&ink_primitives::Key::from(key_bytes)), Some(_)));
            // Clear storage under that key
            clear_contract_storage(&ink_primitives::Key::from(key_bytes));
            
            self.data.migration_performed = true;

            Ok(())
        }

        /// Checks if migration was already performed
        #[ink(message)]
        pub fn migration_performed(&self) -> bool {
            return self.data.migration_performed
        }

        fn current_price(&self) -> Balance {
            let block = self.env().block_number();
            let elapsed = block.saturating_sub(self.data.current_start_block);
            self.average_price()
                .saturating_mul(self.data.sale_multiplier)
                .saturating_sub(self.per_block_reduction().saturating_mul(elapsed.into()))
                .max(self.data.min_price)
        }

        fn per_block_reduction(&self) -> Balance {
            self.average_price()
                .saturating_div(self.data.auction_length.into())
                .max(1u128)
        }

        fn take_payment(&self, from: AccountId, amount: Balance) -> Result<(), Error> {
            build_call::<Environment>()
                .call_type(Call::new().callee(self.data.reward_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(REWARD_BURN_SELECTOR))
                        .push_arg(from)
                        .push_arg(amount),
                )
                .call_flags(CallFlags::default().set_allow_reentry(true))
                .returns::<Result<(), PSP22Error>>()
                .fire()??;

            Ok(())
        }

        fn give_ticket(&self, to: AccountId) -> Result<(), Error> {
            build_call::<Environment>()
                .call_type(Call::new().callee(self.data.ticket_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TRANSFER_TICKET_SELECTOR))
                        .push_arg(to)
                        .push_arg(1u128)
                        .push_arg(DUMMY_DATA),
                )
                .returns::<Result<(), PSP22Error>>()
                .fire()??;

            Ok(())
        }

        fn ticket_balance(&self) -> Result<Balance, Error> {
            let balance = build_call::<Environment>()
                .call_type(Call::new().callee(self.data.ticket_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TICKET_BALANCE_SELECTOR))
                        .push_arg(self.env().account_id()),
                )
                .returns::<Balance>()
                .fire()?;

            Ok(balance)
        }

        fn ensure_role(role: Role) -> Result<(), Error> {
            <Self as AccessControlled>::check_role(
                AccountId::from(ACCESS_CONTROL_PUBKEY),
                Self::env().caller(),
                role,
                |reason| reason.into(),
                Error::MissingRole,
            )
        }

        fn initializer() -> Role {
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Failure to retrieve code hash.");
            Role::Initializer(code_hash)
        }

        fn admin(&self) -> Role {
            Role::Admin(self.env().account_id())
        }

        fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event)
        }
    }
}
