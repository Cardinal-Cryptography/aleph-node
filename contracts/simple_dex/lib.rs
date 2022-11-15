#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::let_unit_value)]

use ink_lang as ink;

/// Simple DEX contract
///
/// This contract is based on Balancer multi asset LP design and all formulas are taken from the Balancer's whitepaper (https://balancer.fi/whitepaper.pdf)
/// It has one pool with PSP22 tokens with equal weights
///
/// Swaps can be performed between all pairs in the pool whitelisted for trading
/// Liquidity provisioning is limited to designated accounts only and works as deposits / withdrawals of arbitrary composition.

#[ink::contract]
mod simple_dex {

    use access_control::{roles::Role, traits::AccessControlled, ACCESS_CONTROL_PUBKEY};
    use game_token::{
        ALLOWANCE_SELECTOR, BALANCE_OF_SELECTOR, TRANSFER_FROM_SELECTOR, TRANSFER_SELECTOR,
    };
    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        CallFlags, DefaultEnvironment, Error as InkEnvError,
    };
    use ink_lang::{
        codegen::{initialize_contract, EmitEvent},
        reflect::ContractEventBase,
    };
    use ink_prelude::{format, string::String, vec, vec::Vec};
    use ink_storage::traits::{PackedLayout, SpreadAllocate, SpreadLayout};
    use openbrush::{contracts::traits::errors::PSP22Error, storage::Mapping};

    type Event = <SimpleDex as ContractEventBase>::Type;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct SwapPair {
        pub from: AccountId,
        pub to: AccountId,
    }

    impl SwapPair {
        pub fn new(from: AccountId, to: AccountId) -> Self {
            Self { from, to }
        }
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum DexError {
        PSP22(PSP22Error),
        InsufficientAllowanceOf(AccountId),
        Arithmethic,
        WrongParameterValue,
        MissingRole(Role),
        InkEnv(String),
        CrossContractCall(String),
        TooMuchSlippage,
        NotEnoughLiquidityOf(AccountId),
        UnsupportedSwapPair(SwapPair),
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

    #[ink(event)]
    pub struct SwapPairAdded {
        #[ink(topic)]
        pair: SwapPair,
    }

    #[ink(event)]
    pub struct SwapPairRemoved {
        #[ink(topic)]
        pair: SwapPair,
    }

    #[ink(event)]
    pub struct Swapped {
        caller: AccountId,
        #[ink(topic)]
        token_in: AccountId,
        #[ink(topic)]
        token_out: AccountId,
        amount_in: Balance,
        amount_out: Balance,
    }

    #[ink(event)]
    pub struct SwapFeeSet {
        #[ink(topic)]
        caller: AccountId,
        swap_fee_percentage: u128,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct SimpleDex {
        pub swap_fee_percentage: u128,
        pub access_control: AccountId,
        // a set of pairs that are availiable for swapping between
        pub swap_pairs: Mapping<SwapPair, ()>,
    }

    impl AccessControlled for SimpleDex {
        type ContractError = DexError;
    }

    impl SimpleDex {
        #[ink(constructor)]
        pub fn new() -> Self {
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
                Self::cross_contract_call_error_handler,
                Self::access_control_error_handler,
            );

            match role_check {
                Ok(_) => initialize_contract(Self::new_init),
                Err(why) => panic!("Could not initialize the contract {:?}", why),
            }
        }

        /// Swaps the a specified amount of one of the pool's PSP22 tokens to another PSP22 token
        /// Calling account needs to give allowance to the DEX contract to spend amount_token_in of token_in on its behalf
        /// before executing this tx.
        #[ink(message)]
        pub fn swap(
            &mut self,
            token_in: AccountId,
            token_out: AccountId,
            amount_token_in: Balance,
            min_amount_token_out: Balance,
        ) -> Result<(), DexError> {
            let this = self.env().account_id();
            let caller = self.env().caller();

            let swap_pair = SwapPair::new(token_in, token_out);
            if !self.swap_pairs.contains(&swap_pair) {
                return Err(DexError::UnsupportedSwapPair(swap_pair));
            }

            // check allowance
            if self.allowance(token_in, caller, this)? < amount_token_in {
                return Err(DexError::InsufficientAllowanceOf(token_in));
            }

            let amount_token_out = self.out_given_in(
                token_in,
                token_out,
                amount_token_in,
                Some(min_amount_token_out),
            )?;

            if amount_token_out < min_amount_token_out {
                // thrown if too much slippage occured before this tx gets executed
                // as a sandwich attack prevention
                return Err(DexError::TooMuchSlippage);
            }

            // transfer token_in from user to the contract
            self.transfer_from_tx(token_in, caller, this, amount_token_in)??;
            // transfer token_out from contract to user
            self.transfer_tx(token_out, caller, amount_token_out)??;

            // emit event
            Self::emit_event(
                self.env(),
                Event::Swapped(Swapped {
                    caller,
                    token_in,
                    token_out,
                    amount_in: amount_token_in,
                    amount_out: amount_token_out,
                }),
            );

            Ok(())
        }

        /// Liquidity deposit
        ///
        /// Can only be performed by an account with a LiquidityProvider role
        /// Caller needs to give at least the passed amount of allowance to the contract to spend the deposited tokens on his behalf
        /// prior to executing this tx
        #[ink(message)]
        pub fn deposit(&mut self, deposits: Vec<(AccountId, Balance)>) -> Result<(), DexError> {
            let this = self.env().account_id();
            let caller = self.env().caller();

            // check role, only designated account can add liquidity
            self.check_role(caller, Role::LiquidityProvider(this))?;

            deposits
                .into_iter()
                .try_for_each(|(token_in, amount)| -> Result<(), DexError> {
                    // transfer token_in from the caller to the contract
                    // will revert if the contract does not have enough allowance from the caller
                    // in which case the whole tx is reverted
                    self.transfer_from_tx(token_in, caller, this, amount)??;
                    Ok(())
                })?;

            Ok(())
        }

        #[ink(message)]
        pub fn withdrawal(
            &mut self,
            withdrawals: Vec<(AccountId, Balance)>,
        ) -> Result<(), DexError> {
            let this = self.env().account_id();
            let caller = self.env().caller();

            // check role, only designated account can remove liquidity
            self.check_role(caller, Role::LiquidityProvider(this))?;

            withdrawals.into_iter().try_for_each(
                |(token_out, amount)| -> Result<(), DexError> {
                    // transfer token_out from the contract to the caller
                    self.transfer_tx(token_out, caller, amount)??;
                    Ok(())
                },
            )?;

            Ok(())
        }

        /// Alters the swap_fee parameter
        ///
        /// Can only be called by the contract's Admin.
        #[ink(message)]
        pub fn set_swap_fee_percentage(
            &mut self,
            swap_fee_percentage: u128,
        ) -> Result<(), DexError> {
            if swap_fee_percentage.gt(&100) {
                return Err(DexError::WrongParameterValue);
            }

            let caller = self.env().caller();
            let this = self.env().account_id();

            self.check_role(caller, Role::Admin(this))?;

            // emit event
            Self::emit_event(
                self.env(),
                Event::SwapFeeSet(SwapFeeSet {
                    caller,
                    swap_fee_percentage,
                }),
            );

            self.swap_fee_percentage = swap_fee_percentage;
            Ok(())
        }

        /// Returns current value of the swap_fee_percentage parameter
        #[ink(message)]
        pub fn swap_fee_percentage(&self) -> Balance {
            self.swap_fee_percentage
        }

        /// Sets access_control to a new contract address
        ///
        /// Potentially very destructive, can only be called by the contract's Owner.
        #[ink(message)]
        pub fn set_access_control(&mut self, access_control: AccountId) -> Result<(), DexError>
        where
            Self: AccessControlled,
        {
            let caller = self.env().caller();
            let this = self.env().account_id();

            self.check_role(caller, Role::Owner(this))?;

            self.access_control = access_control;
            Ok(())
        }

        /// Returns current address of the AccessControl contract that holds the account priviledges for this DEX
        #[ink(message)]
        pub fn access_control(&self) -> AccountId {
            self.access_control
        }

        /// Whitelists a token pair for swapping between
        ///
        /// Token pair is understood as a swap between tokens in one direction
        /// Can only be called by an Admin
        #[ink(message)]
        pub fn add_swap_pair(&mut self, from: AccountId, to: AccountId) -> Result<(), DexError> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            self.check_role(caller, Role::Admin(this))?;

            let pair = SwapPair::new(from, to);
            self.swap_pairs.insert(&pair, &());

            Self::emit_event(self.env(), Event::SwapPairAdded(SwapPairAdded { pair }));

            Ok(())
        }

        /// Blacklists a token pair from swapping
        ///
        /// Token pair is understood as a swap between tokens in one direction
        /// Can only be called by an Admin
        #[ink(message)]
        pub fn remove_swap_pair(&mut self, from: AccountId, to: AccountId) -> Result<(), DexError> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            self.check_role(caller, Role::Admin(this))?;

            let pair = SwapPair::new(from, to);
            self.swap_pairs.remove(&pair);

            Self::emit_event(self.env(), Event::SwapPairRemoved(SwapPairRemoved { pair }));

            Ok(())
        }

        /// Terminates the contract.
        ///
        /// Can only be called by the contract's Owner.
        #[ink(message)]
        pub fn terminate(&mut self) -> Result<(), DexError> {
            let caller = self.env().caller();
            let this = self.env().account_id();
            self.check_role(caller, Role::Owner(this))?;
            self.env().terminate_contract(caller)
        }

        /// Returns own code hash
        #[ink(message)]
        pub fn code_hash(&self) -> Result<Hash, DexError> {
            self.env()
                .own_code_hash()
                .map_err(|why| DexError::InkEnv(format!("Can't retrieve own code hash: {:?}", why)))
        }

        /// Swap trade output given a curve with equal token weights
        ///
        /// swap_fee_percentage (integer) is a percentage of the trade that goes towards the pool
        /// B_0 - (100 * B_0 * B_i) / (100 * (B_i + A_i) -A_i * fee)
        #[ink(message)]
        pub fn out_given_in(
            &self,
            token_in: AccountId,
            token_out: AccountId,
            amount_token_in: Balance,
            min_amount_token_out: Option<Balance>,
        ) -> Result<Balance, DexError> {
            let this = self.env().account_id();
            let balance_token_in = self.balance_of(token_in, this)?;
            let balance_token_out = self.balance_of(token_out, this)?;

            if min_amount_token_out.map_or(false, |x| balance_token_out < x) {
                // throw early if we cannot support this swap anyway due to liquidity being too low
                return Err(DexError::NotEnoughLiquidityOf(token_out));
            }

            let op0 = amount_token_in
                .checked_mul(self.swap_fee_percentage)
                .ok_or(DexError::Arithmethic)?;

            let op1 = balance_token_in
                .checked_add(amount_token_in)
                .and_then(|result| result.checked_mul(100))
                .ok_or(DexError::Arithmethic)?;

            let op2 = op1.checked_sub(op0).ok_or(DexError::Arithmethic)?;

            let op3 = balance_token_in
                .checked_mul(balance_token_out)
                .and_then(|result| result.checked_mul(100))
                .ok_or(DexError::Arithmethic)?;

            let op4 = op3.checked_div(op2).ok_or(DexError::Arithmethic)?;

            balance_token_out
                .checked_sub(op4)
                .ok_or(DexError::Arithmethic)
        }

        fn new_init(&mut self) {
            self.access_control = AccountId::from(ACCESS_CONTROL_PUBKEY);
            self.swap_fee_percentage = 0;
        }

        /// Transfers a given amount of a PSP22 token to a specified using the callers own balance
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

        /// Transfers a given amount of a PSP22 token on behalf of a specified account to another account
        ///
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
                .call_flags(CallFlags::default().set_allow_reentry(true)) // needed for checking allowance before the actual tx
                .returns::<Result<(), PSP22Error>>()
                .fire()
        }

        /// Returns the amount of unused allowance that the token owner has given to the spender
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

        /// Returns DEX balance of a PSP22 token for an account
        fn balance_of(&self, token: AccountId, account: AccountId) -> Result<Balance, InkEnvError> {
            build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(token))
                .exec_input(
                    ExecutionInput::new(Selector::new(BALANCE_OF_SELECTOR)).push_arg(account),
                )
                .returns::<Balance>()
                .fire()
        }

        fn access_control_error_handler(role: Role) -> DexError {
            DexError::MissingRole(role)
        }

        fn cross_contract_call_error_handler(why: InkEnvError) -> DexError {
            DexError::CrossContractCall(format!("Calling access control has failed: {:?}", why))
        }

        fn check_role(&self, account: AccountId, role: Role) -> Result<(), DexError>
        where
            Self: AccessControlled,
        {
            <Self as AccessControlled>::check_role(
                self.access_control,
                account,
                role,
                Self::cross_contract_call_error_handler,
                Self::access_control_error_handler,
            )
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<SimpleDex>,
        {
            emitter.emit_event(event);
        }
    }

    impl Default for SimpleDex {
        fn default() -> Self {
            SimpleDex::new()
        }
    }
}
