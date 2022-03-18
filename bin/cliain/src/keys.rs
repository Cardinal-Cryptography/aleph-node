use aleph_client::{
    create_connection, rotate_keys, rotate_keys_raw_result, set_keys, staking_bond, KeyPair,
    SessionKeys,
};
use log::info;
use primitives::staking::MIN_VALIDATOR_BOND;
use sp_core::Pair;
use substrate_api_client::{AccountId, XtStatus};

pub fn prepare(node: String, key: KeyPair) {
    let connection = create_connection(&node).set_signer(key.clone());
    let controller_account_id = AccountId::from(key.public());
    staking_bond(
        &connection,
        MIN_VALIDATOR_BOND,
        &controller_account_id,
        XtStatus::Finalized,
    );
    let new_keys = rotate_keys(&connection).expect("Failed to retrieve keys");
    set_keys(&connection, new_keys, XtStatus::Finalized);
}

pub fn set_keys_command(node: String, new_keys: String, controller_seed: String) {
    let controller_key =
        KeyPair::from_string(&controller_seed, None).expect("Can't create pair from seed value");
    let connection = create_connection(&node).set_signer(controller_key);
    set_keys(
        &connection,
        SessionKeys::try_from(new_keys).expect("Failed to parse keys"),
        XtStatus::InBlock,
    );
}

pub fn rotate_keys_command(node: String, key: KeyPair) {
    let connection = create_connection(&node).set_signer(key.clone());
    let new_keys = rotate_keys_raw_result(&connection).expect("Failed to retrieve keys");
    info!("Rotated keys: {:?}", new_keys);
}
