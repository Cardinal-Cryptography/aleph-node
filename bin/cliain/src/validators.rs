use aleph_client::RootConnection;
use primitives::CommitteeSeats;
use sp_core::crypto::Ss58Codec;
use substrate_api_client::{AccountId, XtStatus};

/// Change validators to the provided list by calling the provided node.
pub fn change_validators(
    root_connection: RootConnection,
    reserved_validators: Vec<String>,
    non_reserved_validators: Vec<String>,
    reserved_committee_size: Option<u32>,
    non_reserved_committee_size: Option<u32>,
) {
    let convert_vec_of_strings_to_account_ids = |validators_string: Vec<String>| {
        validators_string
            .iter()
            .map(|address| AccountId::from_ss58check(address).expect("Address is valid"))
            .collect()
    };
    let some_vec_or_none = |validators_string: Vec<String>| match validators_string.is_empty() {
        true => None,
        false => Some(convert_vec_of_strings_to_account_ids(validators_string)),
    };

    aleph_client::change_validators(
        &root_connection,
        some_vec_or_none(reserved_validators),
        some_vec_or_none(non_reserved_validators),
        match reserved_committee_size.is_some() || non_reserved_committee_size.is_some() {
            false => None,
            true => Some(CommitteeSeats {
                reserved_seats: reserved_committee_size.unwrap_or(0),
                non_reserved_seats: non_reserved_committee_size.unwrap_or(0),
            }),
        },
        XtStatus::Finalized,
    );
    // TODO we need to check state here whether change members actually succeed
    // not only here, but for all cliain commands
    // see https://cardinal-cryptography.atlassian.net/browse/AZ-699
}
