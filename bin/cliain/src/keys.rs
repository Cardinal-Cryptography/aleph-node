use aleph_client::{staking_bond, set_keys, rotate_keys, create_connection, KeyPair};
use primitives::TOKEN_DECIMALS;
use substrate_api_client::XtStatus;

const TOKEN: u128 = 10u128.pow(TOKEN_DECIMALS);
const VALIDATOR_STAKE: u128 = 25_000 * TOKEN;

pub fn prepare(node: String, key: KeyPair) {
    let connection = create_connection(&node).set_signer(key.clone());
    staking_bond(&connection, VALIDATOR_STAKE, &key, XtStatus::Finalized);
    let new_keys = rotate_keys(&connection).expect("Connection works").expect("Received new keys");
    set_keys(&connection, new_keys, XtStatus::Finalized);
}
