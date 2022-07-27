use aleph_client::{RootConnection, SignedConnection};

use crate::{accounts::{get_sudo_key, get_validators_keys}, Config};

pub fn get_root_connection(config: &Config) -> RootConnection {
    let node = &config.node;
    let sudo = get_sudo_key(config);
    RootConnection::new(node, sudo)
}

pub fn get_signed_connection(config: &Config) -> SignedConnection {
    let node = &config.node;
    let accounts = get_validators_keys(config);
    let sender = accounts.first().expect("Using default accounts").to_owned();
    SignedConnection::new(node, sender)
}