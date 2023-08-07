use aleph_client::{pallets::balances::BalanceUserApi, Balance, SignedConnection, TxStatus};
use primitives::TOKEN;
use subxt::ext::{sp_core::crypto::Ss58Codec, sp_runtime::AccountId32 as SpAccountId};

pub async fn transfer(connection: SignedConnection, amount_in_tokens: u64, to_account: String) {
    let to_account = SpAccountId::from_ss58check(&to_account)
        .expect("Address is valid")
        .into();
    connection
        .transfer(
            to_account,
            amount_in_tokens as Balance * TOKEN,
            TxStatus::Finalized,
        )
        .await
        .unwrap();
}
