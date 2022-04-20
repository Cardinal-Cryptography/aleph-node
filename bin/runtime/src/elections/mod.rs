use pallet_staking::EraIndex;
use primitives::{
    TOKEN,
    SessionIndex
};
use crate::{AccountId, BlockNumber, MembersPerSession, Perbill, SessionPeriod, SessionsPerEra, Runtime, Staking, Rewards, Session, Vec};


fn points_per_block(exposure_total: u128, nr_of_sessions: EraIndex, blocks_per_session: u32, token: u128) -> u32 {
    let total = exposure_total / token;
    let blocks_to_produce_per_era = nr_of_sessions * blocks_per_session;

    Perbill::from_rational(1, blocks_to_produce_per_era) * total as u32
}

fn compute_reward_ratio(validator: AccountId, era: EraIndex, blocks_per_session: u32) -> (u32, u32) {
    let nr_of_sessions = pallet_rewards::ErasParticipatedSessions::<Runtime>::get(&era, &validator);

    // was never elected, get max ratio
    if nr_of_sessions == 0 {
        return (1, 1);
    }

    let produced_blocks = pallet_rewards::ErasBlockProduced::<Runtime>::get(&era, &validator);
    let expected_nr_blocks = nr_of_sessions * blocks_per_session;

    (produced_blocks, expected_nr_blocks)
}

pub struct StakeReward;

impl pallet_authorship::EventHandler<AccountId, BlockNumber> for StakeReward {
    fn note_author(validator: AccountId) {
        let active_era = Staking::active_era().unwrap().index;
        pallet_rewards::ErasBlockProduced::<Runtime>::mutate(&active_era, &validator, |v| {
            *v += 1
        });
        // let exposure = pallet_staking::ErasStakers::<Runtime>::get(active_era, &validator);
        // let number_of_sessions_per_validator = SessionsPerEra::get()
        //     / (MembersPerSession::get() - Staking::invulnerables().len() as u32);
        // let blocks_to_produce_per_session = SessionPeriod::get() / MembersPerSession::get();
        // let points_per_block = points_per_block(exposure.total, number_of_sessions_per_validator, blocks_to_produce_per_session, TOKEN);
        //
        // pallet_staking::Pallet::<Runtime>::reward_by_ids(
        //     [(validator, points_per_block)].into_iter(),
        // );
    }

    fn note_uncle(_author: AccountId, _age: BlockNumber) {}
}


// Choose a subset of all the validators for current era that contains all the
// reserved nodes. Non reserved ones are chosen in consecutive batches for every session
fn rotate() -> Option<Vec<AccountId>> {
    let current_era = Staking::current_era()?;
    if current_era == 0 {
        return None;
    }
    let mut all_validators: Vec<AccountId> =
        pallet_staking::ErasStakers::<Runtime>::iter_key_prefix(current_era).collect();
    // this is tricky: we might change the number of reserved nodes for (current_era + 1) while still
    // in current_era. This is not a problem as long as we enlarge the list.
    // A solution could be to store reserved nodes per era, but this is somewhat cumbersome.
    // There is a ticket to improve that in later iteration
    let mut validators = pallet_staking::Invulnerables::<Runtime>::get();
    all_validators.retain(|v| !validators.contains(v));
    let n_all_validators = all_validators.len();

    let n_validators = MembersPerSession::get() as usize;
    let free_sits = n_validators.checked_sub(validators.len()).unwrap();

    let current_session = Session::current_index() as usize;
    let first_validator = current_session * free_sits;

    validators.extend(
        (first_validator..first_validator + free_sits)
            .map(|i| all_validators[i % n_all_validators].clone()),
    );

    for validator in &validators {
        pallet_rewards::ErasParticipatedSessions::<Runtime>::mutate(&current_era, validator, |v| {
            *v += 1
        });
    }
    Some(validators)
}

fn reward_session_validators() {}

type SM = pallet_session::historical::NoteHistoricalRoot<Runtime, Staking>;
pub struct SampleSessionManager;

impl pallet_session::SessionManager<AccountId> for SampleSessionManager {
    fn new_session(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        SM::new_session(new_index);
        rotate()
    }

    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<AccountId>> {
        SM::new_session_genesis(new_index)
    }

    fn end_session(end_index: SessionIndex) {
        SM::end_session(end_index)
    }

    fn start_session(start_index: SessionIndex) {
        SM::start_session(start_index)
    }
}



#[cfg(test)]
mod tests {
    use crate::elections::points_per_block;

    #[test]
    fn calculates_reward_correctly() {
        assert_eq!(1000, points_per_block(1_000_000, 10, 100, 1));
    }
}