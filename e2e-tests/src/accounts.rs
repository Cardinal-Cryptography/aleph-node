use aleph_client::{keypair_from_string, KeyPair};

use crate::config::Config;

fn get_validator_seed(seed: u32) -> String {
    format!("//{}", seed)
}

// this should be extracted to common code
pub fn get_validators_seeds(config: &Config) -> Vec<String> {
    match config.validators_seeds {
        Some(ref seeds) => seeds.clone(),
        None => (0..config.validators_count)
            .map(get_validator_seed)
            .collect(),
    }
}

pub fn get_validators_keys(config: &Config) -> Vec<KeyPair> {
    accounts_seeds_to_keys(&get_validators_seeds(config))
}

pub fn accounts_seeds_to_keys(seeds: &[String]) -> Vec<KeyPair> {
    seeds
        .iter()
        .map(String::as_str)
        .map(keypair_from_string)
        .collect()
}

pub fn get_sudo_key(config: &Config) -> KeyPair {
    keypair_from_string(&config.sudo_seed)
}

pub struct NodeKeys {
    pub validator_key: KeyPair,
    pub controller_key: KeyPair,
    pub stash_key: KeyPair,
}

impl From<u32> for NodeKeys {
    fn from(seed: u32) -> Self {
        let validator_seed = get_validator_seed(seed);
        NodeKeys::from(validator_seed)
    }
}

impl From<String> for NodeKeys {
    fn from(seed: String) -> Self {
        Self {
            validator_key: keypair_from_string(&seed[..]),
            controller_key: keypair_from_string(&get_validators_controller_seed(seed.clone())[..]),
            stash_key: keypair_from_string(&get_validators_stash_seed(seed.clone())[..]),
        }
    }
}

fn get_validators_controller_seed(seed: String) -> String {
    format!("{}//Controller", seed)
}

fn get_validators_stash_seed(seed: String) -> String {
    format!("{}//stash", seed)
}
