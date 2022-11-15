use aleph_client::waiting::{AlephWaiting, BlockStatus};

use crate::config::Config;

pub async fn finalization(config: &Config) -> anyhow::Result<()> {
    let connection = config.create_root_connection().await;
    connection
        .connection
        .wait_for_block(|n| n >= 1, BlockStatus::Finalized)
        .await;
    Ok(())
}
