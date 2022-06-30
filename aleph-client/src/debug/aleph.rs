use crate::{
    debug::{element_prompt, entry_prompt, pallet_prompt},
    read_storage, AnyConnection,
};
use primitives::AuthorityId;

pub fn print_storage<C: AnyConnection>(connection: &C) {
    let authorities: Vec<AuthorityId> = read_storage(connection, "Aleph", "Authorities");

    println!("{}", pallet_prompt("Aleph"));
    println!("{}", entry_prompt("Authorities"));

    for auth in authorities {
        println!(
            "{}",
            element_prompt(format!("\tAuthority {:?}", auth.to_string()))
        );
    }
}
