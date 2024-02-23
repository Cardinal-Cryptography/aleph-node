use aleph_client::{pallet_feature_control::Feature, pallets::feature_control::FeatureControlApi};
use anyhow::Result;

use crate::config::setup_test;

const FEATURE: Feature = Feature::OnChainVerifier;
const IS_ON: bool = true;
const IS_OFF: bool = false;

#[tokio::test]
pub async fn fresh_chain_has_verifier_enabled() -> Result<()> {
    let config = setup_test();
    let conn = config.get_first_signed_connection().await;

    assert_feature_status(IS_ON, &conn).await;

    Ok(())
}

async fn assert_feature_status<Conn: FeatureControlApi>(active: bool, c: &Conn) {
    assert_eq!(c.is_feature_active(FEATURE, None).await, active)
}
