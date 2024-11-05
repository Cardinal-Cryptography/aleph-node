use aleph_client::{
    utility::BlocksApi,
    waiting::{AlephWaiting, BlockStatus},
};
use anyhow::anyhow;
use log::info;

use crate::config::setup_test;

#[tokio::test]
pub async fn finalization() -> anyhow::Result<()> {
    let config = setup_test();
    let connection = config.create_root_connection().await;
    let wait_blocks = config.test_case_params.finalization_wait.unwrap_or(1);

    let finalized = connection.get_finalized_block_hash().await?;
    info!("Highest finalized block hash = {finalized}");
    let finalized_number = connection
        .get_block_number(finalized)
        .await?
        .ok_or(anyhow!(
            "Failed to retrieve block number for hash {finalized:?}"
        ))?;
    let wait_for_block = finalized_number.saturating_add(wait_blocks);
    info!("Waiting for block {} to be finalized", wait_for_block);

    connection
        .wait_for_block(|n| n >= wait_for_block, BlockStatus::Finalized)
        .await;

    Ok(())
}
