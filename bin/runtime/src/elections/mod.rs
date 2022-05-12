use crate::{
    AccountId, BlockNumber, EraIndex, Perquintill, Runtime, Session, SessionPeriod, SessionsPerEra,
    Staking, Vec,
};
use pallet_elections::MembersPerSession;
use primitives::{SessionIndex, TOKEN};

fn blocks_to_produce_per_session() -> u32 {
    SessionPeriod::get() / MembersPerSession::<Runtime>::get()
}

fn scale_total_exposure(total: u128) -> u128 {
    // to cover minimal possible theoretical stake (25k) and avoid loss of accuracy we need to scale
    100 * total / TOKEN
}

fn scaled_total_exposure(era: u32, validator: &AccountId) -> u128 {
    let total = pallet_staking::ErasStakers::<Runtime>::get(era, validator).total;

    scale_total_exposure(total)
}

fn validator_points_per_block(
    total_exposure_in_tokens: u128,
    sessions_per_era: u32,
    blocks_to_produce_per_session: u32,
) -> u32 {
    (Perquintill::from_rational(1, (blocks_to_produce_per_session * sessions_per_era) as u64)
        * total_exposure_in_tokens) as u32
}

fn reward_for_session_non_committee(committee: &[AccountId]) {
    let active_era = match Staking::active_era() {
        Some(ae) => ae.index,
        _ => return,
    };
    let all_validators = pallet_staking::ErasStakers::<Runtime>::iter_key_prefix(active_era);

    let nr_of_sessions = SessionsPerEra::get();
    let blocks_per_session = blocks_to_produce_per_session();

    let session_rewards = all_validators.into_iter().map(|validator| {
        (
            validator.clone(),
            calculate_adjusted_session_points(
                committee,
                active_era,
                nr_of_sessions,
                blocks_per_session,
                &validator,
                scaled_total_exposure,
            ),
        )
    });

    pallet_staking::Pallet::<Runtime>::reward_by_ids(session_rewards);
}

fn calculate_adjusted_session_points<F: Fn(EraIndex, &T) -> u128, T: Clone + PartialEq>(
    committee: &[T],
    active_era: EraIndex,
    nr_of_sessions: EraIndex,
    blocks_per_session: u32,
    validator: &T,
    get_total_exposure: F,
) -> u32 {
    let participated = committee.contains(validator);

    if participated {
        return 0;
    }

    let total = get_total_exposure(active_era, validator);
    let points_per_block = validator_points_per_block(total, nr_of_sessions, blocks_per_session);

    points_per_block * blocks_per_session
}

pub struct StakeReward;
impl pallet_authorship::EventHandler<AccountId, BlockNumber> for StakeReward {
    fn note_author(validator: AccountId) {
        let active_era = Staking::active_era().unwrap().index;

        let total = scaled_total_exposure(active_era, &validator);
        let sessions_per_era = SessionsPerEra::get();

        let validator_points_per_block =
            validator_points_per_block(total, sessions_per_era, blocks_to_produce_per_session());

        pallet_staking::Pallet::<Runtime>::reward_by_ids(
            [(validator, validator_points_per_block)].into_iter(),
        );
    }

    fn note_uncle(_author: AccountId, _age: BlockNumber) {}
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
    let free_seats = n_validators.checked_sub(reserved.len()).unwrap();
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

// Choose a subset of all the validators for current era that contains all the
// reserved nodes. Non reserved ones are chosen in consecutive batches for every session
fn rotate_committee() -> Option<Vec<AccountId>> {
    let current_era = match Staking::active_era() {
        Some(ae) if ae.index > 0 => ae.index,
        _ => return None,
    };
    let all_validators: Vec<AccountId> =
        pallet_staking::ErasStakers::<Runtime>::iter_key_prefix(current_era).collect();
    let reserved = pallet_elections::ErasReserved::<Runtime>::get();
    let n_validators = pallet_elections::MembersPerSession::<Runtime>::get() as usize;
    let current_session = Session::current_index();

    rotate(
        current_era,
        current_session,
        n_validators,
        all_validators,
        reserved,
    )
}

fn populate_reserved_on_next_era_start(start_index: SessionIndex) {
    let current_era = match Staking::active_era() {
        Some(ae) => ae.index,
        _ => return,
    };
    // this will be populated once for the session `n+1` on the start of the session `n` where session
    // `n+1` starts a new era.
    if let Some(era_index) = Staking::eras_start_session_index(current_era + 1) {
        if era_index == start_index {
            let reserved_validators = pallet_staking::Invulnerables::<Runtime>::get();
            pallet_elections::ErasReserved::<Runtime>::put(reserved_validators);
        }
    }
}

fn adjust_rewards_for_session() {
    match Staking::active_era() {
        Some(ae) if ae.index == 0 => return,
        None => return,
        _ => {}
    }

    let committee = pallet_session::Validators::<Runtime>::get();
    reward_for_session_non_committee(&committee);
}

type SM = pallet_session::historical::NoteHistoricalRoot<Runtime, Staking>;
pub struct CommitteeRotationSessionManager;

impl pallet_session::SessionManager<AccountId> for CommitteeRotationSessionManager {
    fn new_session(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        SM::new_session(new_index);
        // new session is always called before the end_session of the previous session
        // so we need to populate reserved set here not on start_session nor end_session
        let committee = rotate_committee();
        populate_reserved_on_next_era_start(new_index);

        committee
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        SM::new_session_genesis(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        SM::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        SM::start_session(start_index);
        adjust_rewards_for_session();
    }
}

#[cfg(test)]
mod tests {
    use crate::elections::{
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
    fn adjusted_session_points_for_committee_member_is_zero() {
        assert_eq!(
            0,
            calculate_adjusted_session_points(&[1, 2, 3, 4], 1, 5, 30, &1, |_, _| 2_500_000)
        );

        assert_eq!(
            0,
            calculate_adjusted_session_points(&[1, 2, 3, 4], 1, 96, 900, &1, |_, _| {
                60_000_000_000
            })
        );
    }

    #[test]
    fn adjusted_session_points_for_non_committee_member_is_correct() {
        assert_eq!(
            500010,
            calculate_adjusted_session_points(&[1, 2, 3, 4], 1, 5, 30, &5, |_, _| 2_500_000)
        );

        assert_eq!(
            624999600,
            calculate_adjusted_session_points(&[1, 2, 3, 4], 1, 96, 900, &5, |_, _| {
                60_000_000_000
            })
        );

        assert_eq!(
            614583000,
            calculate_adjusted_session_points(&[1, 2, 3, 4], 1, 96, 900, &5, |era, _| {
                match era {
                    0 => 0,
                    1 => 59_000_000_000,
                    _ => 60_000_000_000,
                }
            })
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
