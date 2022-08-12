#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// Smart Contract modelling classic Prisoner's Dilemma problem.
/// To receive rewards, the contract needs to be pre-founded in the first place
#[ink::contract]
mod two_prisoners_dilemma {
    use ink_lang::utils::initialize_contract;
    use ink_prelude::string::{String, ToString};
    use ink_storage::{
        traits::{PackedLayout, SpreadAllocate, SpreadLayout},
        Mapping,
    };
    use scale::{Decode, Encode};

    // For now we harcode number of players to two, to model original dillema
    const NUMBER_OF_PLAYERS: usize = 2;
    pub const TOKEN_DECIMALS: u32 = 12;
    pub const TOKEN: u128 = 10u128.pow(TOKEN_DECIMALS);

    /// A choice of a player during the game
    /// Initially all registered players are in `Waiting` state
    #[derive(Debug, Encode, Decode, Clone, Copy, SpreadLayout, PackedLayout, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    enum Choice {
        Waiting,
        Cooperate,
        Defect,
    }

    /// An event emitted when a reward was claimed and transfer succeeded
    /// From acocunt is always the contract one
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        to: AccountId,
        value: Balance,
    }

    /// An event emitted when a player makes a legitimate choice
    /// legitimate: not duplicate, and from registered players only
    /// Obviously the exact choice is not revealed
    #[ink(event)]
    pub struct PlayerMadeChoice {
        #[ink(topic)]
        player: AccountId,
    }

    /// Written result of the game for a given player
    #[ink(event)]
    pub struct ResultOfTheGame {
        #[ink(topic)]
        player: AccountId,
        result: String,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct TwoPrisonersDilemma {
        /// A reward a cooperating player receives when other one defect
        bad_luck_payoff: u128,

        /// A reward both players receive when they both defect
        defect_payoff: u128,

        /// A reward both players receive when they both cooperate
        cooperation_payoff: u128,

        /// A reward a defecting player gets when other one cooperate
        temptation_payoff: u128,

        /// Players choices, lookup of account to either choice of cooperate or defect
        choices: Mapping<AccountId, Choice>,

        /// How many legitimate votes has been made so far
        /// legitimate: not duplicate, and from registered players only
        vote_count: u32,

        /// Registered player accounts
        players: [AccountId; NUMBER_OF_PLAYERS],

        /// Claimed players rewards
        claimed_rewards: Mapping<AccountId, ()>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// sender is not registered player, either when trying to cooperate or defect
        CallerIsNotPlayer,

        /// player already chosen, ie cooperated or defected
        PlayerAlreadyChosen,

        /// not all players made their choices
        NotAllPlayersMadeTheirChoices,

        /// contract's funds are insufficient to pay out reward to a player
        InsufficientContractFunds,

        /// requested transfer failed, this can be the case if the contract does not
        /// have sufficient free funds or if the transfer would have brought the
        /// contract's balance below minimum balance
        TransferFailed,

        /// a reward has been already claimed
        RewardAlreadyClaimed,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl TwoPrisonersDilemma {
        /// Creates new instance of Prisoner's Dilemma SC with below parameters:
        /// # Arguments
        ///
        /// * `players` - list of exactly 2 accounts what will play the game
        ///
        /// Game has following rewards configured (in tokens):
        /// * `bad_luck_payoff`: 0
        /// * `defect_payoff`: 1
        /// * `cooperation_payoff`: 2,
        /// * `temptation_payoff`: 3.
        #[ink(constructor)]
        pub fn new(players: [AccountId; 2]) -> Self {
            initialize_contract(|contract: &mut Self| {
                contract.players = players.clone();
                for player in players {
                    contract.choices.insert(&player, &Choice::Waiting);
                }

                contract.bad_luck_payoff = 0;
                contract.defect_payoff = TOKEN;
                contract.cooperation_payoff = 2 * TOKEN;
                contract.temptation_payoff = 3 * TOKEN;

                contract.vote_count = 0;
            })
        }

        fn get_callers_current_choice(&self, caller: &AccountId) -> Result<Choice> {
            let current_choice = self.choices.get(caller);
            if current_choice.is_none() {
                return Err(Error::CallerIsNotPlayer);
            }
            Ok(current_choice.unwrap())
        }

        fn get_other_player_choice(&self, this_player: &AccountId) -> Choice {
            let other_player_index: usize = match self.players[0] == *this_player {
                true => 1,
                false => 0,
            };
            return self.choices.get(self.players[other_player_index]).unwrap();
        }

        fn make_choice(&mut self, choice: &Choice) -> Result<()> {
            let caller = Self::env().caller();
            let current_choice = self.get_callers_current_choice(&caller)?;
            if current_choice != Choice::Waiting {
                return Err(Error::PlayerAlreadyChosen);
            }
            self.choices.insert(&caller, choice);
            self.vote_count += 1;
            self.env().emit_event(PlayerMadeChoice { player: caller });

            Ok(())
        }

        fn compute_reward(
            &self,
            this_player_choice: &Choice,
            other_player_choice: &Choice,
        ) -> (Balance, &str) {
            match (this_player_choice, other_player_choice) {
                (&Choice::Cooperate, &Choice::Defect) => (
                    self.bad_luck_payoff,
                    "You co-operated, but other player did not.",
                ),
                (&Choice::Defect, &Choice::Defect) => {
                    (self.defect_payoff, "You and other player both defected.")
                }
                (&Choice::Cooperate, &Choice::Cooperate) => (
                    self.cooperation_payoff,
                    "You and other player both co-operated.",
                ),
                (&Choice::Defect, &Choice::Cooperate) => (
                    self.temptation_payoff,
                    "You defected, but other player did not.",
                ),
                (_, _) => panic!("Both players should made their choices at this point!"),
            }
        }

        /// Make a choice to cooperate
        #[ink(message)]
        pub fn cooperate(&mut self) -> Result<()> {
            self.make_choice(&Choice::Cooperate)
        }

        /// Make a choice to defect
        #[ink(message)]
        pub fn defect(&mut self) -> Result<()> {
            self.make_choice(&Choice::Defect)
        }

        /// How many legitimate votes has been made do far
        /// legitimate: not duplicate, and from registered players only
        #[ink(message)]
        pub fn vote_count(&self) -> u32 {
            self.vote_count
        }

        /// Computes a player's reward based on they choice and other player
        /// Transfers funds to a player's accounts from contract funds
        #[ink(message)]
        pub fn claim_reward(&mut self) -> Result<()> {
            if self.vote_count != NUMBER_OF_PLAYERS as u32 {
                return Err(Error::NotAllPlayersMadeTheirChoices);
            }

            let caller = Self::env().caller();
            if self.claimed_rewards(caller)? == true {
                return Err(Error::RewardAlreadyClaimed);
            }

            let current_choice = self.get_callers_current_choice(&caller)?;
            let other_players_choice = self.get_other_player_choice(&caller);
            let (reward, game_result) = self.compute_reward(&current_choice, &other_players_choice);
            self.env().emit_event(ResultOfTheGame {
                player: caller,
                result: game_result.to_string(),
            });

            if reward > self.env().balance() {
                return Err(Error::InsufficientContractFunds);
            }
            self.env()
                .transfer(caller, reward)
                .map_err(|_| Error::TransferFailed)?;
            self.claimed_rewards.insert(caller, &());
            self.env().emit_event(Transfer {
                to: caller,
                value: reward,
            });

            Ok(())
        }

        /// Returns whether given player claimed already their reward
        #[ink(message)]
        pub fn claimed_rewards(&self, player: AccountId) -> Result<bool> {
            self.get_callers_current_choice(&player)?;
            Ok(self.claimed_rewards.contains(player))
        }
    }

    #[cfg(test)]
    mod tests {
        use ink_lang as ink;

        use super::*;

        fn get_default_test_accounts() -> ink_env::test::DefaultAccounts<ink_env::DefaultEnvironment>
        {
            ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
        }

        fn set_balance(account_id: AccountId, balance: Balance) {
            ink_env::test::set_account_balance::<ink_env::DefaultEnvironment>(account_id, balance)
        }

        fn get_balance(account_id: AccountId) -> Balance {
            ink_env::test::get_account_balance::<ink_env::DefaultEnvironment>(account_id)
                .expect("Cannot get account balance")
        }

        fn contract_id() -> AccountId {
            ink_env::test::callee::<ink_env::DefaultEnvironment>()
        }

        #[ink::test]
        fn given_new_contract_constructor_initialize_values() {
            let accounts = get_default_test_accounts();
            let two_prisoners_dilemma = TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            assert_eq!(two_prisoners_dilemma.bad_luck_payoff, 0);
            assert_eq!(two_prisoners_dilemma.defect_payoff, TOKEN);
            assert_eq!(two_prisoners_dilemma.cooperation_payoff, 2 * TOKEN);
            assert_eq!(two_prisoners_dilemma.temptation_payoff, 3 * TOKEN);
            assert_eq!(two_prisoners_dilemma.vote_count(), 0);
            assert_eq!(
                two_prisoners_dilemma.choices.get(accounts.eve).unwrap(),
                Choice::Waiting
            );
            assert_eq!(
                two_prisoners_dilemma.choices.get(accounts.frank).unwrap(),
                Choice::Waiting
            );
            assert_eq!(two_prisoners_dilemma.choices.get(accounts.charlie), None);
        }

        #[ink::test]
        fn given_player_cooperated_when_cooperated_again_then_error_is_returned() {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));
            assert_eq!(two_prisoners_dilemma.vote_count(), 1);
            assert_eq!(
                two_prisoners_dilemma.cooperate(),
                Err(Error::PlayerAlreadyChosen)
            );
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));
            assert_eq!(two_prisoners_dilemma.vote_count(), 2);
        }

        #[ink::test]
        fn given_non_player_account_when_cooperate_then_error_is_returned() {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.charlie);
            assert_eq!(
                two_prisoners_dilemma.cooperate(),
                Err(Error::CallerIsNotPlayer)
            );
        }

        #[ink::test]
        fn given_player_defected_when_defected_again_then_error_is_returned() {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));
            assert_eq!(two_prisoners_dilemma.vote_count(), 1);
            assert_eq!(
                two_prisoners_dilemma.defect(),
                Err(Error::PlayerAlreadyChosen)
            );
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));
            assert_eq!(two_prisoners_dilemma.vote_count(), 2);
        }

        #[ink::test]
        fn given_non_player_account_when_defect_then_error_is_returned() {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.charlie);
            assert_eq!(
                two_prisoners_dilemma.defect(),
                Err(Error::CallerIsNotPlayer)
            );
        }

        #[ink::test]
        fn given_player_defected_when_trying_to_cooperate_later_then_error_is_returned() {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));
            assert_eq!(two_prisoners_dilemma.vote_count(), 1);
            assert_eq!(
                two_prisoners_dilemma.cooperate(),
                Err(Error::PlayerAlreadyChosen)
            );
        }

        #[ink::test]
        fn given_player_cooperated_when_trying_to_defect_later_then_error_is_returned() {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));
            assert_eq!(two_prisoners_dilemma.vote_count(), 1);
            assert_eq!(
                two_prisoners_dilemma.defect(),
                Err(Error::PlayerAlreadyChosen)
            );
        }

        fn claim_rewards_and_assert_state(
            two_prisoners_dilemma: &mut TwoPrisonersDilemma,
            player_a: AccountId,
            player_b: AccountId,
            expected_reward_player_a: Balance,
            expected_reward_player_b: Balance,
        ) {
            set_balance(contract_id(), 100 * TOKEN);
            assert_eq!(get_balance(player_a), 0);
            assert_eq!(get_balance(player_b), 0);

            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(player_a);
            assert_eq!(two_prisoners_dilemma.claim_reward(), Ok(()));
            assert_eq!(two_prisoners_dilemma.claimed_rewards(player_a), Ok(true));
            assert_eq!(two_prisoners_dilemma.claimed_rewards(player_b), Ok(false));
            assert_eq!(get_balance(player_a), expected_reward_player_a);
            assert_eq!(get_balance(player_b), 0);

            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(player_b);
            assert_eq!(two_prisoners_dilemma.claim_reward(), Ok(()));
            assert_eq!(two_prisoners_dilemma.claimed_rewards(player_a), Ok(true));
            assert_eq!(two_prisoners_dilemma.claimed_rewards(player_b), Ok(true));
            assert_eq!(get_balance(player_a), expected_reward_player_a);
            assert_eq!(get_balance(player_b), expected_reward_player_b);
        }

        #[ink::test]
        fn given_new_contract_when_both_players_cooperate_then_they_both_receive_appropriate_rewards(
        ) {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));

            claim_rewards_and_assert_state(
                &mut two_prisoners_dilemma,
                accounts.eve,
                accounts.frank,
                2 * TOKEN,
                2 * TOKEN,
            );
        }

        #[ink::test]
        fn given_new_contract_when_both_players_defect_then_they_both_receive_appropriate_rewards()
        {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));

            claim_rewards_and_assert_state(
                &mut two_prisoners_dilemma,
                accounts.eve,
                accounts.frank,
                TOKEN,
                TOKEN,
            );
        }

        #[ink::test]
        fn given_new_contract_when_first_player_defect_and_other_cooperate_then_they_both_receive_appropriate_rewards(
        ) {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));

            claim_rewards_and_assert_state(
                &mut two_prisoners_dilemma,
                accounts.eve,
                accounts.frank,
                3 * TOKEN,
                0,
            );

            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(
                two_prisoners_dilemma.claim_reward(),
                Err(Error::RewardAlreadyClaimed)
            );
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(
                two_prisoners_dilemma.claim_reward(),
                Err(Error::RewardAlreadyClaimed)
            );
        }

        #[ink::test]
        fn given_new_contract_when_first_player_cooperate_and_other_defect_then_they_both_receive_appropriate_rewards(
        ) {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(two_prisoners_dilemma.defect(), Ok(()));

            claim_rewards_and_assert_state(
                &mut two_prisoners_dilemma,
                accounts.eve,
                accounts.frank,
                0,
                3 * TOKEN,
            );
        }

        #[ink::test]
        fn given_not_all_players_made_their_choices_when_either_of_players_makes_choice_then_error()
        {
            let accounts = get_default_test_accounts();
            let mut two_prisoners_dilemma =
                TwoPrisonersDilemma::new([accounts.eve, accounts.frank]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(two_prisoners_dilemma.cooperate(), Ok(()));
            assert_eq!(
                two_prisoners_dilemma.claim_reward(),
                Err(Error::NotAllPlayersMadeTheirChoices)
            );
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.frank);
            assert_eq!(
                two_prisoners_dilemma.claim_reward(),
                Err(Error::NotAllPlayersMadeTheirChoices)
            );
        }
    }
}
