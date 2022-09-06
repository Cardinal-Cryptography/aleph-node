#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

use ink_lang as ink;

#[ink::contract]
pub mod marketplace {
    use access_control::{
        traits::AccessControlled, Role, Role::Initializer, ACCESS_CONTROL_PUBKEY,
    };
    use game_token::TRANSFER_FROM_SELECTOR as TRANSFER_FROM_GAME_TOKEN_SELECTOR;
    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        CallFlags,
    };
    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_prelude::{format, string::String};
    use openbrush::contracts::psp22::PSP22Error;
    use ticket_token::{
        BALANCE_OF_SELECTOR as TICKET_BALANCE_SELECTOR,
        TRANSFER_SELECTOR as TRANSFER_TICKET_SELECTOR,
    };

    use crate::marketplace::Error::MissingRole;

    type Event = <Marketplace as ContractEventBase>::Type;

    const DUMMY_DATA: &[u8] = &[0x0];

    #[ink(storage)]
    pub struct Marketplace {
        total_proceeds: Balance,
        tickets_sold: Balance,
        min_price: Balance,
        current_start_block: BlockNumber,
        auction_length: BlockNumber,
        sale_multiplier: Balance,
        ticket_token: AccountId,
        game_token: AccountId,
    }

    #[derive(Clone, Eq, PartialEq, Debug, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        MissingRole(String),
        ContractCall(String),
        PSP22TokenCall(String),
        MarketplaceEmpty,
    }

    #[ink(event)]
    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Buy {
        #[ink(topic)]
        pub account_id: AccountId,
        pub price: Balance,
    }

    #[ink(event)]
    #[derive(Clone, Eq, PartialEq, Debug)]
    pub struct Reset;

    impl Error {
        fn missing_role(role: Role) -> Self {
            MissingRole(format!("{:?}", role))
        }
    }

    impl From<ink_env::Error> for Error {
        fn from(inner: ink_env::Error) -> Self {
            Error::ContractCall(format!("{:?}", inner))
        }
    }

    impl From<PSP22Error> for Error {
        fn from(inner: PSP22Error) -> Self {
            Error::PSP22TokenCall(format!("{:?}", inner))
        }
    }

    impl AccessControlled for Marketplace {
        type ContractError = Error;
    }

    impl Marketplace {
        #[ink(constructor)]
        pub fn new(
            ticket_token: AccountId,
            game_token: AccountId,
            starting_price: Balance,
            min_price: Balance,
            sale_multiplier: Balance,
            auction_length: BlockNumber,
        ) -> Self {
            Self::ensure_role(Self::initializer())
                .unwrap_or_else(|e| panic!("Failed to initialize the contract {:?}", e));

            Marketplace {
                ticket_token,
                game_token,
                min_price,
                sale_multiplier,
                auction_length,
                current_start_block: Self::env().block_number(),
                total_proceeds: starting_price.saturating_div(sale_multiplier),
                tickets_sold: 1,
            }
        }

        fn ensure_role(role: Role) -> Result<(), Error> {
            <Self as AccessControlled>::check_role(
                AccountId::from(ACCESS_CONTROL_PUBKEY),
                Self::env().caller(),
                role,
                |reason| reason.into(),
                |role| Error::missing_role(role),
            )
        }

        fn initializer() -> Role {
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Failure to retrieve code hash.");
            Initializer(code_hash)
        }

        fn admin(&self) -> Role {
            Role::Admin(self.this())
        }

        #[ink(message)]
        pub fn price(&self) -> Balance {
            self.current_price()
        }

        #[ink(message)]
        pub fn sale_multiplier(&self) -> Balance {
            self.sale_multiplier
        }

        #[ink(message)]
        pub fn available_tickets(&self) -> Result<Balance, Error> {
            self.ticket_balance()
        }

        #[ink(message)]
        pub fn min_price(&self) -> Balance {
            self.min_price
        }

        #[ink(message)]
        pub fn game_token(&self) -> AccountId {
            self.game_token
        }

        #[ink(message)]
        pub fn ticket_token(&self) -> AccountId {
            self.ticket_token
        }

        #[ink(message)]
        pub fn buy(&mut self) -> Result<(), Error> {
            if self.ticket_balance()? > 0 {
                let price = self.current_price();
                let account_id = self.env().caller();

                self.take_payment(account_id, price)?;
                self.give_ticket(account_id)?;

                self.total_proceeds = self.total_proceeds.saturating_add(price);
                self.tickets_sold = self.tickets_sold.saturating_add(1);
                self.current_start_block = self.env().block_number();
                Self::emit_event(self.env(), Event::Buy(Buy { price, account_id }));

                Ok(())
            } else {
                Err(Error::MarketplaceEmpty)
            }
        }

        #[ink(message)]
        pub fn reset(&mut self) -> Result<(), Error> {
            Self::ensure_role(self.admin())?;

            self.current_start_block = self.env().block_number();
            Self::emit_event(self.env(), Event::Reset(Reset {}));

            Ok(())
        }

        fn current_price(&self) -> Balance {
            let block = self.env().block_number();
            let elapsed = block.saturating_sub(self.current_start_block.into());
            self.average_price()
                .saturating_mul(self.sale_multiplier)
                .saturating_sub(self.per_block_reduction().saturating_mul(elapsed.into()))
                .max(self.min_price)
        }

        fn per_block_reduction(&self) -> Balance {
            self.average_price()
                .saturating_div(self.auction_length.into())
                .max(1u128)
        }

        fn average_price(&self) -> Balance {
            self.total_proceeds.saturating_div(self.tickets_sold)
        }

        fn take_payment(&self, from: AccountId, amount: Balance) -> Result<(), Error> {
            build_call::<Environment>()
                .call_type(Call::new().callee(self.game_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TRANSFER_FROM_GAME_TOKEN_SELECTOR))
                        .push_arg(from)
                        .push_arg(self.this())
                        .push_arg(amount)
                        .push_arg(DUMMY_DATA),
                )
                .call_flags(CallFlags::default().set_allow_reentry(true))
                .returns::<Result<(), PSP22Error>>()
                .fire()??;

            Ok(())
        }

        fn give_ticket(&self, to: AccountId) -> Result<(), Error> {
            build_call::<Environment>()
                .call_type(Call::new().callee(self.ticket_token))
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
                .call_type(Call::new().callee(self.ticket_token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TICKET_BALANCE_SELECTOR))
                        .push_arg(self.this()),
                )
                .returns::<Balance>()
                .fire()?;

            Ok(balance)
        }

        fn this(&self) -> AccountId {
            self.env().account_id()
        }

        fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event)
        }
    }
}
