use crate::{
    debug::{element_prompt, entry_prompt, pallet_prompt},
    Connection,
};
use primitives::AuthorityId;

pub fn print_storage(connection: &Connection) {
    let authorities: Vec<AuthorityId> = connection
        .get_storage_value("Aleph", "Authorities", None)
        .unwrap()
        .unwrap();

    println!("{}", pallet_prompt("Aleph"));
    println!("{}", entry_prompt("Authorities"));

    for auth in authorities {
        println!(
            "{}",
            element_prompt(format!("\tAuthority {:?}", auth.to_string()))
        );
    }
}
