use aleph_client::{
    create_connection, staking_bond, staking_force_new_era, staking_set_staking_limits,
    staking_validate, KeyPair,
};
use primitives::TOKEN;
use sp_core::crypto::Ss58Codec;
use sp_core::Pair;
use substrate_api_client::{AccountId, XtStatus};

pub fn bond_command(
    node: String,
    initial_stake_in_tokens: u32,
    controller_account: String,
    stash_seed: String,
) {
    let controller_account =
        AccountId::from_ss58check(&controller_account).expect("Address is valid");
    let stash_key = KeyPair::from_string(&format!("//{}", stash_seed), None)
        .expect("Can't create pair from seed value");

    let connection = create_connection(&node).set_signer(stash_key);

    let initial_stake = initial_stake_in_tokens as u128 * TOKEN;
    staking_bond(
        &connection,
        initial_stake,
        &controller_account,
        XtStatus::Finalized,
    );
}

pub fn validate_command(node: String, controller_seed: String, commission_percentage: u8) {
    let controller_key = KeyPair::from_string(&format!("//{}", controller_seed), None)
        .expect("Can't create pair from seed value");
    let connection = create_connection(&node).set_signer(controller_key);
    staking_validate(&connection, commission_percentage, XtStatus::Finalized);
}

pub fn set_staking_limits_command(
    node: String,
    root_key: KeyPair,
    minimal_nominator_stake_tokens: u64,
    minimal_validator_stake_tokens: u64,
) {
    let root_connection = create_connection(&node).set_signer(root_key);
    staking_set_staking_limits(
        &root_connection,
        minimal_nominator_stake_tokens as u128 * TOKEN,
        minimal_validator_stake_tokens as u128 * TOKEN,
        XtStatus::Finalized,
    );
}

pub fn force_new_era_command(node: String, root_key: KeyPair) {
    let root_connection = create_connection(&node).set_signer(root_key);
    staking_force_new_era(&root_connection, XtStatus::Finalized);
}
