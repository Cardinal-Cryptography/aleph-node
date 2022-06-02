#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// This is the YellowButton
/// Rewards are distributed for extending the life of the button for as long as possible:
/// user_score = deadline - now
/// Pressiah gets 50% of tokens
/// the game is played until TheButton dies

// DONE : contract holds ERC20 funds
// DONE : contract distributes funds to all accounts that participated (according to a formula)
// e.g. :
// - 50% go to the Pressiah
// - rest is distributed proportionally to how long has a given user extended TheButtons life for
// DONE : add sybil protection (only whitelisted accounts can participate)
// - DONE add / remove whitelisted accounts
// - DONE add access-control
// DONE : add getters
// TODO : refactor access control to an Ownable trait

#[allow(non_camel_case_types)]
#[ink::contract]
mod yellow_button {

    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        DefaultEnvironment, Error as InkEnvError,
    };
    use ink_lang::{codegen::EmitEvent, reflect::ContractEventBase};
    use ink_prelude::{string::String, vec::Vec};
    use ink_storage::{traits::SpreadAllocate, Mapping};

    /// Error types
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Returned if given account already pressed The Button
        AlreadyParticipated,
        /// Returned if button is pressed after the deadline
        AfterDeadline,
        /// Account not whitelisted to play
        NotWhitelisted,
        /// Returned when an account which is not the owner calls a method with access control
        NotOwner,
        /// Returned if a call to another contract has failed
        ContractCall(String),
    }

    /// Result type
    pub type Result<T> = core::result::Result<T, Error>;
    /// Event type
    type Event = <YellowButton as ContractEventBase>::Type;

    impl From<InkEnvError> for Error {
        fn from(e: InkEnvError) -> Self {
            match e {
                InkEnvError::Decode(_e) => {
                    Error::ContractCall(String::from("Contract call failed due to Decode error"))
                }
                InkEnvError::CalleeTrapped => Error::ContractCall(String::from(
                    "Contract call failed due to CalleeTrapped error",
                )),
                InkEnvError::CalleeReverted => Error::ContractCall(String::from(
                    "Contract call failed due to CalleeReverted error",
                )),
                InkEnvError::KeyNotFound => Error::ContractCall(String::from(
                    "Contract call failed due to KeyNotFound error",
                )),
                InkEnvError::_BelowSubsistenceThreshold => Error::ContractCall(String::from(
                    "Contract call failed due to _BelowSubsistenceThreshold error",
                )),
                InkEnvError::TransferFailed => Error::ContractCall(String::from(
                    "Contract call failed due to TransferFailed error",
                )),
                InkEnvError::_EndowmentTooLow => Error::ContractCall(String::from(
                    "Contract call failed due to _EndowmentTooLow error",
                )),
                InkEnvError::CodeNotFound => Error::ContractCall(String::from(
                    "Contract call failed due to CodeNotFound error",
                )),
                InkEnvError::NotCallable => Error::ContractCall(String::from(
                    "Contract call failed due to NotCallable error",
                )),
                InkEnvError::Unknown => {
                    Error::ContractCall(String::from("Contract call failed due to Unknown error"))
                }
                InkEnvError::LoggingDisabled => Error::ContractCall(String::from(
                    "Contract call failed due to LoggingDisabled error",
                )),
                InkEnvError::EcdsaRecoveryFailed => Error::ContractCall(String::from(
                    "Contract call failed due to EcdsaRecoveryFailed error",
                )),
                #[cfg(any(feature = "std", test, doc))]
                InkEnvError::OffChain(_e) => {
                    Error::ContractCall(String::from("Contract call failed due to OffChain error"))
                }
            }
        }
    }

    /// Defines the storage
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct YellowButton {
        /// access control
        owner: AccountId,
        /// How long does TheButton live for?
        button_lifetime: u32,
        /// is The Button dead
        is_dead: bool,
        /// block number at which the game ends
        deadline: u32,
        /// Stores a mapping between user accounts and the number of blocks they extended The Buttons life for
        presses: Mapping<AccountId, u32>,
        /// stores keys to `presses` because Mapping is not an Iterator. Heap-allocated! so we might need Map<u32, AccountId>
        press_accounts: Vec<AccountId>,
        /// stores total sum of user scores
        total_scores: u32,
        /// stores the last account that pressed The Button
        last_presser: Option<AccountId>,
        /// block number of the last press
        last_press: u32,
        /// AccountId of the ERC20 ButtonToken instance on-chain
        button_token: AccountId,
        /// accounts whitelisted to play the game
        can_play: Mapping<AccountId, bool>,
    }

    /// Event emitted when TheButton is pressed
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonPressed {
        #[ink(topic)]
        from: AccountId,
        when: u32,
        new_deadline: u32,
    }

    /// Event emitted when TheButton owner is changed
    #[ink(event)]
    #[derive(Debug)]
    pub struct OwnershipTransferred {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
    }

    /// Event emitted when TheButton is created
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonCreated {
        #[ink(topic)]
        start: u32,
        #[ink(topic)]
        deadline: u32,
    }

    /// Even emitted when button death is triggered    
    #[ink(event)]
    #[derive(Debug)]
    pub struct ButtonDeath;

    // /// NOTE: emit all contract events via this enum
    // /// they cannot be used directly
    // /// as they will be conflated with events imported from button-token
    // pub enum YellowButtonEvent {
    //     ButtonPressed(ButtonPressed),
    //     OwnershipTransferred(OwnershipTransferred),
    //     ButtonCreated(ButtonCreated),
    //     ButtonDeath(ButtonDeath),
    // }

    // impl YellowButtonEvent {
    //     fn emit<EE: EmitEvent<YellowButton>>(self, x: EE) {
    //         match self {
    //             YellowButtonEvent::ButtonPressed(event) => x.emit_event(event),
    //             YellowButtonEvent::OwnershipTransferred(event) => x.emit_event(event),
    //             YellowButtonEvent::ButtonCreated(event) => x.emit_event(event),
    //             YellowButtonEvent::ButtonDeath(event) => x.emit_event(event),
    //         }
    //     }
    // }

    impl YellowButton {
        /// Returns the buttons status
        #[ink(message)]
        pub fn is_dead(&self) -> bool {
            self.is_dead
        }

        /// Returns the current deadline
        #[ink(message)]
        pub fn deadline(&self) -> u32 {
            self.deadline
        }

        /// Returns the user score
        #[ink(message)]
        pub fn score_of(&self, user: AccountId) -> u32 {
            self.presses.get(&user).unwrap_or(0)
        }

        /// Returns whether given account can play
        #[ink(message)]
        pub fn can_play(&self, user: AccountId) -> bool {
            self.can_play.get(&user).unwrap_or(false)
        }

        /// Returns the account id that pressed as last
        #[ink(message)]
        pub fn last_presser(&self) -> Option<AccountId> {
            self.last_presser
        }

        fn emit_event<EE: EmitEvent<YellowButton>>(x: EE, event: Event) {
            x.emit_event(event);
        }

        /// Constructor
        #[ink(constructor)]
        #[allow(non_camel_case_types)]
        pub fn new(button_token: AccountId, button_lifetime: u32) -> Self {
            ink_lang::utils::initialize_contract(|contract: &mut Self| {
                let now = Self::env().block_number();
                let caller = Self::env().caller();
                let deadline = now + button_lifetime;

                contract.owner = caller;
                contract.is_dead = false;
                contract.button_lifetime = button_lifetime;
                contract.deadline = deadline;
                contract.button_token = button_token;

                let event = Event::ButtonCreated(ButtonCreated {
                    start: now,
                    deadline,
                });

                Self::emit_event(Self::env(), event)
            })
        }

        /// End of the game logic
        /// distributes the rewards to the participants
        fn death(&mut self) -> Result<()> {
            self.is_dead = true;

            let this = self.env().account_id();
            let button_token = self.button_token;

            let total_balance = build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(button_token).gas_limit(5000))
                .transferred_value(self.env().transferred_value())
                .exec_input(
                    ExecutionInput::new(
                        Selector::new([0, 0, 0, 2]), // balance_of
                    )
                    .push_arg(this),
                )
                .returns::<Balance>()
                .fire()?;

            // Pressiah gets 50% of supply
            let pressiah_reward = total_balance / 2;
            if let Some(pressiah) = self.last_presser {
                let _ = build_call::<DefaultEnvironment>()
                    .call_type(Call::new().callee(button_token).gas_limit(5000))
                    .transferred_value(self.env().transferred_value())
                    .exec_input(
                        ExecutionInput::new(
                            Selector::new([0, 0, 0, 4]), // transfer
                        )
                        .push_arg(pressiah)
                        .push_arg(pressiah_reward),
                    )
                    .returns::<()>()
                    .fire()?;
            }

            let total = self.total_scores;
            let remaining_balance = total_balance - pressiah_reward;
            // rewards are distributed to participants proportionally to their score
            let _ = self
                .press_accounts
                .iter()
                .try_for_each(|account_id| -> Result<()> {
                    if let Some(score) = self.presses.get(account_id) {
                        let reward = (score / total) as u128 * remaining_balance;

                        // transfer amount
                        return Ok(build_call::<DefaultEnvironment>()
                            .call_type(Call::new().callee(button_token).gas_limit(5000))
                            .transferred_value(self.env().transferred_value())
                            .exec_input(
                                ExecutionInput::new(
                                    Selector::new([0, 0, 0, 4]), // transfer
                                )
                                .push_arg(account_id)
                                .push_arg(reward),
                            )
                            .returns::<()>()
                            .fire()?);
                    }
                    Ok(())
                });

            let event = Event::ButtonDeath(ButtonDeath {});
            Self::emit_event(Self::env(), event);

            Ok(())
        }

        /// Whitelists given AccountId to participate in the game
        ///
        /// returns an error if called by someone else but the owner
        #[ink(message)]
        pub fn allow(&mut self, player: AccountId) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }

            self.can_play.insert(player, &true);
            Ok(())
        }

        /// Whitelists an array of accounts to participate in the game
        ///
        /// returns an error if called by someone else but the owner
        #[ink(message)]
        pub fn bulk_allow(&mut self, players: Vec<AccountId>) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }

            players.iter().for_each(|player| {
                self.can_play.insert(player, &true);
            });

            Ok(())
        }

        /// Blacklists given AccountId from participating in the game
        ///
        /// returns an error if called by someone else but the owner
        #[ink(message)]
        pub fn disallow(&mut self, player: AccountId) -> Result<()> {
            let caller = Self::env().caller();
            if caller != self.owner {
                return Err(Error::NotOwner);
            }
            self.can_play.insert(player, &false);
            Ok(())
        }

        /// terminates the contract
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

            let event = Event::OwnershipTransferred(OwnershipTransferred { from: caller, to });
            Self::emit_event(Self::env(), event);

            Ok(())
        }

        /// Button press logic
        #[ink(message)]
        pub fn press(&mut self) -> Result<()> {
            if self.is_dead {
                return Err(Error::AfterDeadline);
            }

            let now = self.env().block_number();
            if now >= self.deadline {
                // trigger TheButton's death
                // at this point is is after the deadline but the death event has not yet been triggered
                // to distribute the awards
                // the last account to click the button in this state will pay for all the computations
                // but that should be OK (similar to paying for distributing staking rewards)
                return self.death();
            }

            let caller = self.env().caller();
            if self.presses.get(&caller).is_some() {
                return Err(Error::AlreadyParticipated);
            }

            if !self.can_play.get(&caller).unwrap_or(false) {
                return Err(Error::NotWhitelisted);
            }

            // record press
            // score is the number of blocks the button life was extended for
            // this incentivizes pressing as late as possible in the game (but not too late)
            let score = now - self.last_press;
            self.presses.insert(&caller, &score);
            self.press_accounts.push(caller);
            // another
            self.last_presser = Some(caller);
            self.last_press = now;
            self.total_scores += score;
            // reset button lifetime
            self.deadline = now + self.button_lifetime;

            // emit event
            let event = Event::ButtonPressed(ButtonPressed {
                from: caller,
                when: now,
                new_deadline: self.deadline,
            });
            Self::emit_event(Self::env(), event);

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use button_token::ButtonToken;
        use ink_lang as ink;

        #[ink::test]
        fn distributing_rewards() {
            let erc20 = ButtonToken::new(1000);
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();

            // Get contract address
            let address = ink_env::account_id::<ink_env::DefaultEnvironment>();
            println!("{:?}", address);
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(address);
            // alice deploys the game
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.alice);
            let game = YellowButton::new(address, 900);

            // TODO : created event
            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            // assert_eq!(1, emitted_events.len());

            let button_created_event = &emitted_events[1];

            let decoded_event: Event =
                <Event as scale::Decode>::decode(&mut &button_created_event.data[..])
                    .expect("Can't decode as Event");

            match decoded_event {
                Event::ButtonCreated(button_created_event) => {
                    println!("{:?}", button_created_event)
                }
                _ => todo!(),
            }

            // if let Event::ButtonCreated(ButtonCreated { .. }) = decoded_event {}

            // println!("{:?}", decoded_event);
        }
    }
}
