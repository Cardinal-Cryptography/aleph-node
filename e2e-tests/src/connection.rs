use aleph_client::SignedConnection;

use crate::{accounts::get_validators_keys, Config};

/// Get a `SignedConnection` where the signer is the first validator.
pub fn get_signed_connection(config: &Config) -> SignedConnection {
    let node = &config.node;
    let accounts = get_validators_keys(config);
    let sender = accounts.first().expect("Using default accounts").to_owned();
    SignedConnection::new(node, sender)
}
