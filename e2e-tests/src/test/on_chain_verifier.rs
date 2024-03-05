use aleph_client::{
    pallet_feature_control::Feature,
    pallets::{contract::ContractsUserApi, feature_control::FeatureControlApi},
    sp_weights::weight_v2::Weight,
    TxStatus,
};
use anyhow::Result;

use crate::config::setup_test;

const FEATURE: Feature = Feature::OnChainVerifier;
const IS_ON: bool = true;
const IS_OFF: bool = false;
const GAS_LIMIT: Weight = Weight {
    ref_time: 5_000_000_000,
    proof_size: 0,
};

#[tokio::test]
pub async fn fresh_chain_has_verifier_enabled() -> Result<()> {
    let config = setup_test();
    let conn = config.get_first_signed_connection().await;

    assert_feature_status(IS_ON, &conn).await;
    assert_contracts_can_use_verifier(&conn).await;

    Ok(())
}

async fn assert_feature_status<Conn: FeatureControlApi>(active: bool, c: &Conn) {
    assert_eq!(c.is_feature_active(FEATURE, None).await, active)
}

async fn assert_contracts_can_use_verifier<Conn: ContractsUserApi>(c: &Conn) {
    c.instantiate_with_code(
        compile_contract(),
        0,
        GAS_LIMIT,
        None,
        vec![],
        vec![],
        TxStatus::Finalized,
    )
    .await
    .unwrap();
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
