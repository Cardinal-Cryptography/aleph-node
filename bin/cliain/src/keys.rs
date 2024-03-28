use aleph_client::{
    pallets::{
        author::AuthorRpc,
        session::{SessionApi, SessionUserApi},
        staking::StakingUserApi,
    },
    primitives::AlephNodeSessionKeys as SessionKeys,
    AccountId, Connection, RootConnection, SignedConnection, Ss58Codec, TxStatus,
};
use hex::ToHex;
use log::{error, info};
use primitives::staking::MIN_VALIDATOR_BOND;
use serde_json::json;

pub async fn prepare_keys(connection: RootConnection) -> anyhow::Result<()> {
    connection
        .bond(MIN_VALIDATOR_BOND, TxStatus::Finalized)
        .await
        .unwrap();
    let new_keys = connection.author_rotate_keys().await?;
    connection.set_keys(new_keys, TxStatus::Finalized).await?;
    Ok(())
}

pub async fn set_keys(connection: SignedConnection, new_keys: String) {
    connection
        .set_keys(SessionKeys::try_from(new_keys).unwrap(), TxStatus::InBlock)
        .await
        .unwrap();
}

pub async fn rotate_keys(connection: Connection) {
    match connection.author_rotate_keys().await {
        Ok(new_keys) => info!(
            "Keys rotated, use the following in set_keys: {}{}",
            new_keys.aura.0 .0.encode_hex::<String>(),
            new_keys.aleph.0 .0.encode_hex::<String>()
        ),
        Err(e) => error!("Failed to rotate keys: {}.", e),
    }
}

pub async fn next_session_keys(connection: Connection, account_id: String) {
    let account_id = AccountId::from_ss58check(&account_id).expect("Address is valid");
    match connection.get_next_session_keys(account_id, None).await {
        Some(keys) => {
            let keys_json = json!({
                "aura": "0x".to_owned() + keys.aura.0.0.encode_hex::<String>().as_str(),
                "aleph": "0x".to_owned() + keys.aleph.0.0.encode_hex::<String>().as_str(),
            });
            println!("{}", serde_json::to_string_pretty(&keys_json).unwrap());
        }
        None => error!("No keys set for the specified account."),
    }
}
