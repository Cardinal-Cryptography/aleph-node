use aleph_client::{balances_transfer, create_connection, KeyPair};
use primitives::TOKEN;
use sp_core::crypto::Ss58Codec;
use sp_core::Pair;
use substrate_api_client::{AccountId, XtStatus};

pub fn transfer_command(
    node: String,
    sender_seed: String,
    amount_in_tokens: u64,
    to_account: String,
) {
    let sender_key = KeyPair::from_string(&format!("//{}", sender_seed), None)
        .expect("Can't create pair from seed value");
    let connection = create_connection(&node).set_signer(sender_key);
    let to_account = AccountId::from_ss58check(&to_account).expect("Address is valid");
    balances_transfer(
        &connection,
        &to_account,
        amount_in_tokens as u128 * TOKEN,
        XtStatus::Finalized,
    );
}
