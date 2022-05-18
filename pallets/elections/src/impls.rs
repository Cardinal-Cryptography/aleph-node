use crate::{Config, ErasReserved, MembersPerSession, Pallet, SessionValidatorBlockCount};
use frame_election_provider_support::sp_arithmetic::Perquintill;
use frame_support::{pallet_prelude::Get, traits::Currency};
use primitives::TOKEN;
use sp_staking::{EraIndex, SessionIndex};
use sp_std::vec::Vec;

fn scale_total_exposure(total: u128) -> u128 {
    // to cover minimal possible theoretical stake (25k) and avoid loss of accuracy we need to scale
    100 * total / TOKEN
}

fn validator_points_per_block(
    total_exposure_in_tokens: u128,
    sessions_per_era: u32,
    blocks_to_produce_per_session: u32,
) -> u32 {
    // this rounds to the nearest integer
    (Perquintill::from_rational(1, (blocks_to_produce_per_session * sessions_per_era) as u64)
        * total_exposure_in_tokens) as u32
}

fn calculate_adjusted_session_points(
    nr_of_sessions: EraIndex,
    blocks_per_session: u32,
    blocks_created: u32,
    total: u128,
) -> u32 {
    let points_per_block = validator_points_per_block(total, nr_of_sessions, blocks_per_session);

    points_per_block * blocks_created
}

fn rotate<T: Clone + PartialEq>(
    current_era: EraIndex,
    current_session: SessionIndex,
    n_validators: usize,
    all_validators: Vec<T>,
    reserved: Vec<T>,
) -> Option<Vec<T>> {
    if current_era == 0 {
        return None;
    }

    let validators_without_reserved: Vec<_> = all_validators
        .into_iter()
        .filter(|v| !reserved.contains(v))
        .collect();
    let n_all_validators_without_reserved = validators_without_reserved.len();

    // The validators for the committee at the session `n` are chosen as follow:
    // 1. Reserved validators are always chosen.
    // 2. Given non-reserved list of validators the chosen ones are from the range:
    // `n * free_seats` to `(n + 1) * free_seats` where free_seats is equal to free number of free
    // seats in the committee after reserved nodes are added.
    let free_seats = n_validators.saturating_sub(reserved.len());
    let first_validator = current_session as usize * free_seats;

    let committee =
        reserved
            .into_iter()
            .chain((first_validator..first_validator + free_seats).map(|i| {
                validators_without_reserved[i % n_all_validators_without_reserved].clone()
            }))
            .collect();

    Some(committee)
}

impl<T> Pallet<T>
    where
        T: Config + pallet_session::Config + pallet_staking::Config,
        <<T as pallet_staking::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: Into<u128>,
        <T as pallet_session::Config>::ValidatorId: From<T::AccountId>,
        <T as pallet_session::Config>::ValidatorId: Into<T::AccountId>,
{
    fn get_committee_and_non_committee(current_era: EraIndex) -> (Vec<T::AccountId>, Vec<T::AccountId>) {
        let committee: Vec<T::AccountId> = pallet_session::Validators::<T>::get().into_iter().map(|a| a.into()).collect();
        let non_committee = pallet_staking::ErasStakers::<T>::iter_key_prefix(current_era)
            .filter(|a| !committee.contains(a))
            .collect();

        (committee, non_committee)
    }

    fn blocks_to_produce_per_session() -> u32 {
        T::SessionPeriod::get() / MembersPerSession::<T>::get()
    }

    fn scaled_total_exposure(era: u32, validator: &T::AccountId) -> u128 {
        let total = pallet_staking::ErasStakers::<T>::get(era, validator).total;

        scale_total_exposure(total.into())
    }

    fn reward_for_session_non_committee(
        non_committee: Vec<T::AccountId>,
        active_era: EraIndex,
        nr_of_sessions: SessionIndex,
        blocks_per_session: u32,
    ) -> impl IntoIterator<Item=(T::AccountId, u32)> {
        non_committee.into_iter().map(move |validator| {
            let total = Self::scaled_total_exposure(active_era, &validator);
            (
                validator,
                calculate_adjusted_session_points(
                    nr_of_sessions,
                    blocks_per_session,
                    blocks_per_session,
                    total,
                ),
            )
        })
    }

    fn reward_for_session_committee(
        committee: Vec<T::AccountId>,
        active_era: EraIndex,
        nr_of_sessions: SessionIndex,
        blocks_per_session: u32,
    ) -> impl IntoIterator<Item=(T::AccountId, u32)> {
        committee.into_iter().map(move |validator| {
            let total = Self::scaled_total_exposure(active_era, &validator);
            let blocks_created = SessionValidatorBlockCount::<T>::get(&validator);
            (
                validator,
                calculate_adjusted_session_points(
                    nr_of_sessions,
                    blocks_per_session,
                    blocks_created,
                    total,
                ),
            )
        })
    }

    // Choose a subset of all the validators for current era that contains all the
    // reserved nodes. Non reserved ones are chosen in consecutive batches for every session
    fn rotate_committee() -> Option<Vec<T::AccountId>> {
        let current_era = match pallet_staking::ActiveEra::<T>::get() {
            Some(ae) if ae.index > 0 => ae.index,
            _ => return None,
        };
        let all_validators: Vec<T::AccountId> =
            pallet_staking::ErasStakers::<T>::iter_key_prefix(current_era).collect();
        let reserved = ErasReserved::<T>::get();
        let n_validators = MembersPerSession::<T>::get() as usize;
        let current_session = pallet_session::Pallet::<T>::current_index();

        rotate(
            current_era,
            current_session,
            n_validators,
            all_validators,
            reserved,
        )
    }

    fn populate_reserved_on_next_era_start(start_index: SessionIndex) {
        let current_era = match pallet_staking::ActiveEra::<T>::get() {
            Some(ae) => ae.index,
            _ => return,
        };
        // this will be populated once for the session `n+1` on the start of the session `n` where session
        // `n+1` starts a new era.
        if let Some(era_index) = pallet_staking::ErasStartSessionIndex::<T>::get(current_era + 1) {
            if era_index == start_index {
                let reserved_validators = pallet_staking::Invulnerables::<T>::get();
                ErasReserved::<T>::put(reserved_validators);
            }
        }
    }

    fn adjust_rewards_for_session() {
        let active_era = match pallet_staking::ActiveEra::<T>::get() {
            Some(ae) if ae.index > 0 => ae.index,
            _ => return,
        };

        let (committee, non_committee) = Self::get_committee_and_non_committee(active_era);
        let nr_of_sessions = T::SessionsPerEra::get();
        let blocks_per_session = Self::blocks_to_produce_per_session();

        let rewards =
            Self::reward_for_session_non_committee(non_committee, active_era, nr_of_sessions, blocks_per_session)
                .into_iter()
                .chain(Self::reward_for_session_committee(committee, active_era, nr_of_sessions, blocks_per_session).into_iter());

        pallet_staking::Pallet::<T>::reward_by_ids(rewards);
    }
}

impl<T> pallet_authorship::EventHandler<T::AccountId, T::BlockNumber> for Pallet<T>
    where
        T: Config + pallet_session::Config + pallet_staking::Config,
        <<T as pallet_staking::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: Into<u128>,
        <T as pallet_session::Config>::ValidatorId: From<T::AccountId>,
        <T as pallet_session::Config>::ValidatorId: Into<T::AccountId>, {
    fn note_author(validator: T::AccountId) {
        SessionValidatorBlockCount::<T>::mutate(&validator, |count| {
            *count += 1;
        });
    }

    fn note_uncle(_author: T::AccountId, _age: T::BlockNumber) {}
}

impl<T> pallet_session::SessionManager<T::AccountId> for Pallet<T>
    where
        T: Config + pallet_session::Config + pallet_staking::Config,
        <<T as pallet_staking::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: Into<u128>,
        <T as pallet_session::Config>::ValidatorId: From<T::AccountId>,
        <T as pallet_session::Config>::ValidatorId: Into<T::AccountId>, {
    fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <T as Config>::SessionManager::new_session(new_index);
        // new session is always called before the end_session of the previous session
        // so we need to populate reserved set here not on start_session nor end_session
        let committee = Self::rotate_committee();
        Self::populate_reserved_on_next_era_start(new_index);

        committee
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        <T as Config>::SessionManager::new_session_genesis(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        <T as Config>::SessionManager::end_session(end_index);
        Self::adjust_rewards_for_session();

        // clear block count
        SessionValidatorBlockCount::<T>::remove_all(None);
    }

    fn start_session(start_index: SessionIndex) {
        <T as Config>::SessionManager::start_session(start_index);
    }
}

#[cfg(test)]
mod tests {
    use crate::impls::{
        calculate_adjusted_session_points, rotate, scale_total_exposure, validator_points_per_block,
    };
    use primitives::TOKEN;
    use std::collections::VecDeque;

    #[test]
    fn given_minimal_possible_stake_then_points_per_block_are_calculated_correctly() {
        assert_eq!(16667, validator_points_per_block(2_500_000, 5, 30));
        assert_eq!(26042, validator_points_per_block(2_500_000, 96, 1));
        assert_eq!(29, validator_points_per_block(2_500_000, 96, 900));
    }

    #[test]
    fn given_maximal_possible_stake_then_points_per_block_are_calculated_correctly() {
        assert_eq!(625000000, validator_points_per_block(60_000_000_000, 96, 1));
        assert_eq!(694444, validator_points_per_block(60_000_000_000, 96, 900));
    }

    #[test]
    fn given_minimal_possible_stake_then_total_exposure_is_calculated_correctly() {
        assert_eq!(2_500_000, scale_total_exposure(25_000 * TOKEN));
    }

    #[test]
    fn given_maximal_possible_stake_then_total_exposure_is_calculated_correctly() {
        assert_eq!(60_000_000_000, scale_total_exposure(600_000_000 * TOKEN));
    }

    #[test]
    fn given_era_zero_when_rotating_committee_then_committee_is_empty() {
        assert_eq!(None, rotate(0, 0, 4, (0..10).collect(), vec![1, 2, 3, 4]));
    }

    #[test]
    fn adjusted_session_points_all_blocks_created_are_calculated_correctly() {
        assert_eq!(
            500010,
            calculate_adjusted_session_points(5, 30, 30, 2_500_000)
        );

        assert_eq!(
            624999600,
            calculate_adjusted_session_points(96, 900, 900, 60_000_000_000)
        );

        assert_eq!(
            614583000,
            calculate_adjusted_session_points(96, 900, 900, 59_000_000_000)
        );
    }

    #[test]
    fn adjusted_session_points_more_than_all_blocks_created_are_calculated_correctly() {
        assert_eq!(
            2 * 500010,
            calculate_adjusted_session_points(5, 30, 2 * 30, 2_500_000)
        );

        assert_eq!(
            3 * 624999600,
            calculate_adjusted_session_points(96, 900, 3 * 900, 60_000_000_000)
        );

        assert_eq!(
            615265870,
            calculate_adjusted_session_points(96, 900, 901, 59_000_000_000)
        );
    }

    #[test]
    fn given_non_zero_era_and_prime_number_of_validators_when_rotating_committee_then_rotate_is_correct(
    ) {
        let all_validators: Vec<_> = (0..101).collect();
        let reserved: Vec<_> = (0..11).collect();
        let total_validators = 53;
        let mut rotated_free_seats_validators: VecDeque<_> = (11..101).collect();

        for session_index in 0u32..100u32 {
            let mut expected_rotated_free_seats = vec![];
            for _ in 0..total_validators - reserved.len() {
                let first = rotated_free_seats_validators.pop_front().unwrap();
                expected_rotated_free_seats.push(first);
                rotated_free_seats_validators.push_back(first);
            }
            let mut expected_rotated_committee = reserved.clone();
            expected_rotated_committee.append(&mut expected_rotated_free_seats);
            assert_eq!(
                expected_rotated_committee,
                rotate(
                    1,
                    session_index,
                    total_validators,
                    all_validators.clone(),
                    reserved.clone(),
                )
                .expect("Expected non-empty rotated committee!")
            );
        }
    }
}
