use aleph_client::{create_connection, rotate_keys, set_keys, staking_bond, KeyPair};
use primitives::staking::MIN_VALIDATOR_BOND;
use substrate_api_client::XtStatus;

pub fn prepare(node: String, ssl: bool, key: KeyPair) {
    let connection = create_connection(&node, ssl).set_signer(key.clone());
    staking_bond(&connection, MIN_VALIDATOR_BOND, &key, XtStatus::Finalized);
    let new_keys = rotate_keys(&connection)
        .expect("Connection works")
        .expect("Received new keys");
    set_keys(&connection, new_keys, XtStatus::Finalized);
}
