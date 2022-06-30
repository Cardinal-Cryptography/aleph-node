use crate::{
    debug::{element_prompt, entry_prompt, pallet_prompt},
    read_storage, AnyConnection,
};
use primitives::AuthorityId;

pub fn print_storage<C: AnyConnection>(connection: &C) {
    let members: Vec<AuthorityId> = read_storage(connection, "Elections", "Members");

    println!("{}", pallet_prompt("Elections"));
    println!("{}", entry_prompt("Members"));

    for member in members {
        println!(
            "{}",
            element_prompt(format!("\tMember {:?}", member.to_string()))
        );
    }
}
