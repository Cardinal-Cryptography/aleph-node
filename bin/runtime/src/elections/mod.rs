use crate::{
    AccountId, BlockNumber, EraIndex, MembersPerSession, Perbill, Runtime, Session, SessionPeriod,
    SessionsPerEra, Staking, Vec,
};
use primitives::{SessionIndex, TOKEN};

fn total_exposure(era: EraIndex, validator: &AccountId) -> u32 {
    let total = pallet_staking::ErasStakers::<Runtime>::get(era, &validator).total;

    (total / TOKEN) as u32
}

fn points_per_block(total: u32, sessions_per_era: u32, blocks_to_produce_per_session: u32) -> u32 {
    Perbill::from_rational(1, blocks_to_produce_per_session * sessions_per_era) * total as u32
}

fn reward_for_session_non_committee(session: SessionIndex) {
    let active_era = match Staking::active_era() {
        Some(ae) => ae.index,
        _ => return,
    };
    let all_validators = pallet_staking::ErasStakers::<Runtime>::iter_key_prefix(active_era);

    let nr_of_sessions = SessionsPerEra::get();

    let session_rewards = all_validators.into_iter().filter_map(|validator| {
        let participated =
            pallet_rewards::SessionParticipated::<Runtime>::get(&session, &validator);

        if participated {
            return None;
        }

        let total = total_exposure(active_era, &validator);
        let session_points = points_per_block(total, nr_of_sessions, 1);

        Some((validator, session_points))
    });

    pallet_staking::Pallet::<Runtime>::reward_by_ids(session_rewards);
}

pub struct StakeReward;
impl pallet_authorship::EventHandler<AccountId, BlockNumber> for StakeReward {
    fn note_author(validator: AccountId) {
        let active_era = Staking::active_era().unwrap().index;

        let total = total_exposure(active_era, &validator);
        let sessions_per_era = SessionsPerEra::get();
        let blocks_to_produce_per_session = SessionPeriod::get() / MembersPerSession::get();
        let points_per_block =
            points_per_block(total, sessions_per_era, blocks_to_produce_per_session);

        pallet_staking::Pallet::<Runtime>::reward_by_ids(
            [(validator, points_per_block)].into_iter(),
        );
    }

    fn note_uncle(_author: AccountId, _age: BlockNumber) {}
}

// Choose a subset of all the validators for current era that contains all the
// reserved nodes. Non reserved ones are chosen in consecutive batches for every session
fn rotate() -> Option<Vec<AccountId>> {
    let current_era = match Staking::active_era() {
        Some(ae) if ae.index > 0 => ae.index,
        _ => return None,
    };
    let mut all_validators: Vec<AccountId> =
        pallet_staking::ErasStakers::<Runtime>::iter_key_prefix(current_era).collect();

    let mut validators = pallet_elections::ErasReserved::<Runtime>::get();
    all_validators.retain(|v| !validators.contains(v));
    let n_all_validators = all_validators.len();

    let n_validators = MembersPerSession::get() as usize;
    let free_seats = n_validators.checked_sub(validators.len()).unwrap();

    // The validators for the committee at the session `n` are chosen as follow:
    // 1. Reserved validators are always chosen.
    // 2. Given non-reserved list of validators the chosen ones are from the range:
    // `n * free_seats` to `(n + 1) * free_seats` where free_seats is equal to free number of free
    // seats in the committee after reserved nodes are added.
    let current_session = Session::current_index() as usize;
    let first_validator = current_session * free_seats;

    validators.extend(
        (first_validator..first_validator + free_seats)
            .map(|i| all_validators[i % n_all_validators].clone()),
    );

    Some(validators)
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
            let reserved = pallet_staking::Invulnerables::<Runtime>::get();
            pallet_elections::ErasReserved::<Runtime>::put(reserved);
        }
    }
}

fn mark_participation(committee: &Option<Vec<AccountId>>, session: SessionIndex) {
    let committee = match committee {
        Some(c) => c,
        None => return,
    };

    for validator in committee {
        pallet_rewards::SessionParticipated::<Runtime>::insert(&session, &validator, true);
    }
}

type SM = pallet_session::historical::NoteHistoricalRoot<Runtime, Staking>;
pub struct SampleSessionManager;

impl pallet_session::SessionManager<AccountId> for SampleSessionManager {
    fn new_session(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        SM::new_session(new_index);

        let committee = rotate();
        // new session is always called before the end_session of the previous session
        // so we need to populate reserved set here not on start_session nor end_session
        populate_reserved_on_next_era_start(new_index);
        mark_participation(&committee, new_index);

        committee
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        SM::new_session_genesis(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        SM::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        reward_for_session_non_committee(start_index);
        SM::start_session(start_index)
    }
}

#[cfg(test)]
mod tests {
    use crate::elections::points_per_block;

    #[test]
    fn calculates_reward_correctly() {
        assert_eq!(1000, points_per_block(1_000_000, 10, 100));
    }
}
