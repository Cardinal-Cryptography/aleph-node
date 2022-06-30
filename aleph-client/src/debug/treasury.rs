use crate::{
    debug::{element_prompt, entry_prompt, pallet_prompt},
    read_storage_or_else, AnyConnection,
};
use log::trace;
use pallet_treasury::{Proposal, ProposalIndex};
use sp_core::crypto::AccountId32;
use substrate_api_client::Balance;

pub fn print_storage<C: AnyConnection>(connection: &C) {
    let connection = connection.as_connection();
    let proposal_count: u32 = read_storage_or_else(&connection, "Treasury", "ProposalCount", || 0);
    let approvals: Vec<ProposalIndex> =
        read_storage_or_else(&connection, "Treasury", "Approvals", Vec::new);

    println!("{}", pallet_prompt("Treasury"));
    println!("{}: {}", entry_prompt("ProposalCount"), proposal_count);
    println!();
    println!("{}", entry_prompt("Approvals"));
    for x in approvals {
        println!(
            "{}",
            element_prompt(format!("Proposal id {} was approved ", x))
        );
    }
    println!();
    println!("{}", entry_prompt("Proposals"));
    for x in 0..=proposal_count {
        let p: Option<Proposal<AccountId32, Balance>> = connection
            .get_storage_map("Treasury", "Proposals", x, None)
            .unwrap();

        if let Some(p) = p {
            println!("{}", element_prompt(format!("\tProposalId {}: {:?}", x, p)));
        } else {
            trace!("No proposal with id {:?} in the storage", x)
        }
    }
    println!();
}
