use sp_core::Pair;
use sp_runtime::AccountId32;
use substrate_api_client::Balance;

use crate::config::Config;
use crate::{Connection, KeyPair};

pub fn keypair_from_string(seed: &str) -> KeyPair {
    KeyPair::from_string(seed, None).expect("Can't create pair from seed value")
}

pub fn default_account_seeds() -> Vec<String> {
    ["//Damian", "//Hansu", "//Tomasz", "//Zbyszko"]
        .into_iter()
        .map(String::from)
        .collect()
}

pub fn accounts_from_seeds(seeds: &Option<Vec<String>>) -> Vec<KeyPair> {
    match seeds {
        Some(seeds) => seeds
            .iter()
            .map(String::as_str)
            .map(keypair_from_string)
            .collect(),
        None => default_account_seeds()
            .iter()
            .map(String::as_str)
            .map(keypair_from_string)
            .collect(),
    }
}

pub fn get_sudo(config: &Config) -> KeyPair {
    match &config.sudo {
        Some(seed) => keypair_from_string(seed),
        None => accounts_from_seeds(&Some(default_account_seeds()))[0].clone(),
    }
}

pub fn get_free_balance(account: &AccountId32, connection: &Connection) -> Balance {
    connection.get_account_data(account).unwrap().unwrap().free
}
