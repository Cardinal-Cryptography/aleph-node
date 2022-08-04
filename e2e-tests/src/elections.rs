use std::{collections::HashSet, iter::empty};

use aleph_client::{get_validators_for_session, AnyConnection};
use primitives::{CommitteeSeats, EraValidators, SessionIndex};
use substrate_api_client::AccountId;

pub fn get_members_for_session<C: AnyConnection>(
    connection: &C,
    seats: CommitteeSeats,
    era_validators: &EraValidators<AccountId>,
    session: SessionIndex,
) -> (Vec<AccountId>, Vec<AccountId>) {
    let reserved_members_for_session =
        get_members_subset_for_session(seats.reserved_seats, &era_validators.reserved, session);
    let non_reserved_members_for_session = get_members_subset_for_session(
        seats.non_reserved_seats,
        &era_validators.non_reserved,
        session,
    );

    let reserved_members_bench =
        get_bench_members(&era_validators.reserved, &reserved_members_for_session);
    let non_reserved_members_bench = get_bench_members(
        &era_validators.non_reserved,
        &non_reserved_members_for_session,
    );
    let members_bench = empty()
        .chain(reserved_members_bench)
        .chain(non_reserved_members_bench)
        .collect();

    let members_active: Vec<_> = empty()
        .chain(reserved_members_for_session)
        .chain(non_reserved_members_for_session)
        .collect();

    let members_active_set: HashSet<_> = members_active.iter().cloned().collect();
    let network_members: HashSet<_> = get_validators_for_session(connection, session)
        .into_iter()
        .collect();

    assert_eq!(members_active_set, network_members);

    (members_active, members_bench)
}

fn get_members_subset_for_session(
    nodes_per_session: u32,
    era_validators: &[AccountId],
    session: SessionIndex,
) -> Vec<AccountId> {
    let non_reserved_len = era_validators.len();
    let free_seats = nodes_per_session - non_reserved_len as u32;

    let mut non_reserved = Vec::new();

    for i in (free_seats * session)..(free_seats * (session + 1)) {
        non_reserved.push(era_validators[i as usize % non_reserved_len].clone());
    }

    non_reserved
}

fn get_bench_members(all_members: &[AccountId], members_active: &[AccountId]) -> Vec<AccountId> {
    all_members
        .iter()
        .filter(|account_id| !members_active.contains(account_id))
        .cloned()
        .collect::<Vec<_>>()
}
