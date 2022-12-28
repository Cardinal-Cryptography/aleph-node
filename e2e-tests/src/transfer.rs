use aleph_client::{create_connection, AccountId, Client, KeyPair, Pair, SignedConnection};

use crate::{accounts::get_validators_raw_keys, config::Config};

async fn setup(config: &Config) -> (Client, KeyPair, AccountId) {
    let accounts = get_validators_raw_keys(config);
    let (from, to) = (
        KeyPair::new(accounts[0].clone()),
        KeyPair::new(accounts[1].clone()),
    );
    let to = AccountId::from(to.signer().public());
    (create_connection(&config.node).await, from, to)
}

pub async fn setup_for_transfer(config: &Config) -> (SignedConnection, AccountId) {
    let (connection, from, to) = setup(config).await;
    (SignedConnection::from_connection(connection, from), to)
}
