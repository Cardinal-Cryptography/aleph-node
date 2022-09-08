#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod simple_dex {

    use access_control::{traits::AccessControlled, Role, ACCESS_CONTROL_PUBKEY};
    use game_token::{
        ALLOWANCE_SELECTOR, BALANCE_OF_SELECTOR, TRANSFER_FROM_SELECTOR, TRANSFER_SELECTOR,
    };
    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        CallFlags, DefaultEnvironment, Environment as EnvironmentTrait, Error as InkEnvError,
    };
    use ink_lang::{
        codegen::{initialize_contract, EmitEvent},
        reflect::ContractEventBase,
    };
    use ink_prelude::{format, string::String, vec};
    use ink_storage::{traits::SpreadAllocate, Mapping};
    use openbrush::contracts::traits::errors::PSP22Error;

    // Event type
    // type Event = <SimpleDex as ContractEventBase>::Type;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum DexError {
        PSP22(PSP22Error),
        // NotEnoughBalanceOf(AccountId),
        // ArithmethicError,
        InsufficientAllowanceOf(AccountId),
        // InsufficientTransferredValue,
        Arithmethic,
        MissingRole(Role),
        InkEnv(String),
        CrossContractCall(String),
        NativeTransfer,
    }

    impl From<PSP22Error> for DexError {
        fn from(e: PSP22Error) -> Self {
            DexError::PSP22(e)
        }
    }

    impl From<InkEnvError> for DexError {
        fn from(why: InkEnvError) -> Self {
            DexError::InkEnv(format!("{:?}", why))
        }
    }

    // #[ink(event)]
    // #[derive(Debug)]
    // pub struct DexEvt {
    //     #[ink(topic)]
    //     a: AccountId,
    // }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct SimpleDex {
        /// total supply of pool shares
        pub total_liquidity: u128,
        /// tracks pool shares per account
        pub liquidity: Mapping<AccountId, u128>,
        pub swap_fee: u128,
        /// access control contract
        pub access_control: AccountId,
        pub ubik: AccountId,
        pub cyberiad: AccountId,
        pub lono: AccountId,
    }

    impl AccessControlled for SimpleDex {
        type ContractError = DexError;
    }

    impl SimpleDex {
        #[ink(constructor)]
        pub fn new(ubik: AccountId, cyberiad: AccountId, lono: AccountId) -> Self {
            let caller = Self::env().caller();
            let code_hash = Self::env()
                .own_code_hash()
                .expect("Called new on a contract with no code hash");
            let required_role = Role::Initializer(code_hash);
            let access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);

            let role_check = <Self as AccessControlled>::check_role(
                access_control,
                caller,
                required_role,
                |why: InkEnvError| {
                    DexError::CrossContractCall(format!(
                        "Calling access control has failed: {:?}",
                        why
                    ))
                },
                DexError::MissingRole,
            );

            match role_check {
                Ok(_) => {
                    initialize_contract(|contract| Self::new_init(contract, ubik, cyberiad, lono))
                }
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        // mesages

        /// swap the transferred amount of native token to one of the pools PSP22 tokens
        #[ink(message, payable)]
        pub fn native_to_token(&mut self, token_out: AccountId) -> Result<(), DexError> {
            let amount_token_in = self.env().transferred_value();
            // we use the balance of the native token just before the exchange tx
            let balance_token_in = self
                .env()
                .balance()
                .checked_sub(amount_token_in)
                .ok_or(DexError::Arithmethic)?;

            let swap_fee = self.swap_fee;
            let caller = self.env().caller();
            let this = self.env().account_id();

            let balance_token_out = self.balance_of(token_out, this)?;

            let amount_token_out = Self::out_given_in(
                amount_token_in,
                balance_token_in,
                balance_token_out,
                swap_fee,
            )?;

            // transfer token_out from contract to user
            self.transfer_tx(token_out, caller, amount_token_out)??;

            // TOOD : emit event

            Ok(())
        }

        /// swap the a specified amount of pools token to the native token
        /// calling account needs to give allowance to the DEX contract to spend amount_token_in on it's behalf
        #[ink(message)]
        pub fn token_to_native(
            &mut self,
            token_in: AccountId,
            amount_token_in: u128,
        ) -> Result<(), DexError> {
            let caller = self.env().caller();
            let balance_token_out = self.env().balance();
            let swap_fee = self.swap_fee;
            let this = self.env().account_id();
            let balance_token_in = self.balance_of(token_in, this)?;
            let amount_token_out = Self::out_given_in(
                amount_token_in,
                balance_token_in,
                balance_token_out,
                swap_fee,
            )?;

            //  check allowance
            if self.allowance(token_in, caller, this)? < amount_token_in {
                return Err(DexError::InsufficientAllowanceOf(token_in));
            }

            // transfer token_in from the user to the contract
            self.transfer_from_tx(token_in, caller, this, amount_token_in)??;

            self.env()
                .transfer(caller, amount_token_out)
                .map_err(|_| DexError::NativeTransfer)?;

            // TOOD : emit event

            Ok(())
        }

        /// swap the a specified amount of one pool token to another token
        /// calling account needs to give allowance to the DEX contract to spend amount_token_in on it's behalf
        #[ink(message)]
        pub fn token_to_token(
            &mut self,
            token_in: AccountId,
            token_out: AccountId,
            amount_token_in: u128,
        ) -> Result<(), DexError> {
            let swap_fee = self.swap_fee;
            let this = Self::env().account_id();
            let caller = Self::env().caller();

            let balance_token_in = self.balance_of(token_in, this)?;
            let balance_token_out = self.balance_of(token_out, this)?;

            let amount_token_out = Self::out_given_in(
                amount_token_in,
                balance_token_in,
                balance_token_out,
                swap_fee,
            )?;

            // check allowance
            if self.allowance(token_in, caller, this)? < amount_token_in {
                return Err(DexError::InsufficientAllowanceOf(token_in));
            }

            // transfer token_in from user to the contract
            self.transfer_from_tx(token_in, caller, this, amount_token_in)??;
            // transfer token_out from contract to user
            self.transfer_tx(token_out, caller, amount_token_out)??;

            Ok(())
        }

        // END: mesages

        fn new_init(&mut self, ubik: AccountId, cyberiad: AccountId, lono: AccountId) {
            self.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            self.ubik = ubik;
            self.cyberiad = cyberiad;
            self.lono = lono;
        }

        /// transfers a given amount of a PSP22 token to a specified using the contracts own balance
        fn transfer_tx(
            &self,
            token: AccountId,
            to: AccountId,
            amount: Balance,
        ) -> Result<Result<(), PSP22Error>, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TRANSFER_SELECTOR))
                        .push_arg(to)
                        .push_arg(amount)
                        .push_arg(vec![0x0]),
                )
                .returns::<Result<(), PSP22Error>>()
                .fire()
        }

        /// transfers a given amount of a PSP22 token on behalf of a specified account to another account
        /// Will revert if not enough allowance was given to the caller prior to executing this tx
        fn transfer_from_tx(
            &self,
            token: AccountId,
            from: AccountId,
            to: AccountId,
            amount: Balance,
        ) -> Result<Result<(), PSP22Error>, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(token))
                .exec_input(
                    ExecutionInput::new(Selector::new(TRANSFER_FROM_SELECTOR))
                        .push_arg(from)
                        .push_arg(to)
                        .push_arg(amount)
                        .push_arg(vec![0x0]),
                )
                .call_flags(CallFlags::default().set_allow_reentry(true))
                .returns::<Result<(), PSP22Error>>()
                .fire()
        }

        /// returns unused allowance that the owner has given to spender
        fn allowance(
            &self,
            token: AccountId,
            owner: AccountId,
            spender: AccountId,
        ) -> Result<Balance, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(token))
                .exec_input(
                    ExecutionInput::new(Selector::new(ALLOWANCE_SELECTOR))
                        .push_arg(owner)
                        .push_arg(spender),
                )
                .returns::<Balance>()
                .fire()
        }

        /// returns DEX balance of a PSP22 token for an account
        fn balance_of(&self, token: AccountId, account: AccountId) -> Result<Balance, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(token))
                .exec_input(
                    ExecutionInput::new(Selector::new(BALANCE_OF_SELECTOR)).push_arg(account),
                )
                .returns::<Balance>()
                .fire()
        }

        fn out_given_in(
            amount_token_in: u128,
            balance_token_in: u128,
            balance_token_out: u128,
            swap_fee: u128,
        ) -> Result<u128, DexError> {
            let op1 = 1u128.checked_sub(swap_fee).ok_or(DexError::Arithmethic)?;

            let op2 = amount_token_in
                .checked_mul(op1)
                .ok_or(DexError::Arithmethic)?;

            let op3 = balance_token_in
                .checked_add(op2)
                .ok_or(DexError::Arithmethic)?;

            let op4 = balance_token_in
                .checked_div(op3)
                .ok_or(DexError::Arithmethic)?;

            let op5 = 1u128.checked_sub(op4).ok_or(DexError::Arithmethic)?;

            balance_token_out
                .checked_mul(op5)
                .ok_or(DexError::Arithmethic)
        }

        // fn emit_event<EE>(emitter: EE, event: Event)
        // where
        //     EE: EmitEvent<SimpleDex>,
        // {
        //     emitter.emit_event(event);
        // }

        /// Returns own code hash
        #[ink(message)]
        pub fn code_hash(&self) -> Result<Hash, DexError> {
            self.env()
                .own_code_hash()
                .map_err(|why| DexError::InkEnv(format!("Can't retrieve own code hash: {:?}", why)))
        }
    }
}
