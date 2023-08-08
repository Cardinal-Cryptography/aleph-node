use aleph_client::{
    account_from_ss58check, pallets::balances::BalanceUserApi, Balance, SignedConnection, TxStatus,
};
use primitives::TOKEN;

pub async fn transfer(connection: SignedConnection, amount_in_tokens: u64, to_account: String) {
    let to_account = account_from_ss58check(&to_account).expect("Address is valid");
    connection
        .transfer(
            to_account,
            amount_in_tokens as Balance * TOKEN,
            TxStatus::Finalized,
        )
        .await
        .unwrap();
}
