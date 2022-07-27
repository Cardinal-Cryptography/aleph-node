use aleph_client::{account_from_keypair, KeyPair};
use primitives::SessionIndex;
use substrate_api_client::AccountId;

use crate::{accounts::get_validators_keys, Config};

pub fn get_reserved_validators(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[0..2].to_vec()
}

pub fn get_non_reserved_validators(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[2..].to_vec()
}

pub fn get_non_reserved_validators_for_session(
    config: &Config,
    session: SessionIndex,
) -> Vec<AccountId> {
    // Test assumption
    const FREE_SEATS: u32 = 2;

    let mut non_reserved = vec![];

    let non_reserved_nodes_order_from_runtime = get_non_reserved_validators(config);
    let non_reserved_nodes_order_from_runtime_len = non_reserved_nodes_order_from_runtime.len();

    for i in (FREE_SEATS * session)..(FREE_SEATS * (session + 1)) {
        non_reserved.push(
            non_reserved_nodes_order_from_runtime
                [i as usize % non_reserved_nodes_order_from_runtime_len]
                .clone(),
        );
    }

    non_reserved.iter().map(account_from_keypair).collect()
}
