use std::fmt::Debug;

use aleph_client::{
    pallet_feature_control::Feature,
    pallets::{
        contract::ContractsUserApi,
        feature_control::{FeatureControlApi, FeatureControlSudoApi},
    },
    sp_weights::weight_v2::Weight,
    utility::BlocksApi,
    AccountId, TxStatus,
};
use anyhow::Result;
use codec::Encode;

use crate::config::setup_test;

const FEATURE: Feature = Feature::OnChainVerifier;
const IS_ON: bool = true;
const IS_OFF: bool = false;
const GAS_LIMIT: Weight = Weight {
    ref_time: 50_000_000_000,
    proof_size: 50_000_000_000,
};

#[derive(Debug, Encode)]
pub struct VerifyArgs {
    pub verification_key_hash: sp_core::H256,
    pub proof: Vec<u8>,
    pub public_input: Vec<u8>,
}

#[tokio::test]
pub async fn fresh_chain_has_verifier_enabled() -> Result<()> {
    let config = setup_test();
    let conn = config.get_first_signed_connection().await;

    assert_feature_status(IS_ON, &conn).await;
    let contract_address = deploy_contract(&conn, None).await?;
    call_contract(&conn, contract_address).await?;

    Ok(())
}

#[tokio::test]
pub async fn verifier_can_be_disabled() -> Result<()> {
    let config = setup_test();
    let conn = config.create_root_connection().await;

    let contract_address = deploy_contract(&conn, Some(1)).await?;

    conn.disable_feature(FEATURE, TxStatus::Finalized).await?;

    assert_feature_status(IS_OFF, &conn).await;
    assert_code_was_rejected(call_contract(&conn, contract_address).await);
    assert_code_was_rejected(deploy_contract(&conn, Some(2)).await);

    Ok(())
}

async fn assert_feature_status<Conn: FeatureControlApi>(active: bool, c: &Conn) {
    assert_eq!(c.is_feature_active(FEATURE, None).await, active)
}

fn assert_code_was_rejected<T: Debug>(result: Result<T>) {
    assert!(result.is_err());
    assert!(format!("{:?}", result).contains("Pallet error Contracts::CodeRejected"))
}

async fn deploy_contract<Conn: ContractsUserApi + BlocksApi>(
    c: &Conn,
    salt: Option<u8>,
) -> Result<AccountId> {
    let tx_info = c
        .instantiate_with_code(
            compile_contract(),
            0,
            GAS_LIMIT,
            None,
            vec![],
            vec![salt.unwrap_or_default()],
            TxStatus::Finalized,
        )
        .await?;
    let address = c
        .get_tx_events(tx_info)
        .await?
        .find_first::<aleph_client::api::contracts::events::Instantiated>()?
        .unwrap()
        .contract;
    Ok(address.0)
}

async fn call_contract<Conn: ContractsUserApi + BlocksApi>(
    c: &Conn,
    contract_address: AccountId,
) -> Result<()> {
    c.call(
        contract_address,
        0,
        GAS_LIMIT,
        None,
        extension_input(),
        TxStatus::Finalized,
    )
    .await
    .map(|_| ())
}

fn compile_contract() -> Vec<u8> {
    let path = [
        std::env::var("CARGO_MANIFEST_DIR")
            .as_deref()
            .unwrap_or("e2e-tests"),
        "/resources/snark_verifying.wat",
    ]
    .concat();
    wat::parse_file(path).expect("Failed to parse wat file")
}

fn extension_input() -> Vec<u8> {
    (41u32 << 16 | 0u32)
        .to_le_bytes()
        .into_iter()
        .chain(
            VerifyArgs {
                verification_key_hash: sp_core::H256::zero(),
                proof: vec![],
                public_input: vec![],
            }
            .encode(),
        )
        .collect()
}
