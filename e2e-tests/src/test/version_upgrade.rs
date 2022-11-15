use aleph_client::{
    pallets::{aleph::AlephSudoApi, session::SessionApi},
    utility::BlocksApi,
    waiting::{AlephWaiting, BlockStatus},
    TxStatus,
};
use primitives::SessionIndex;

use crate::Config;

const UPGRADE_TO_VERSION: u32 = 1;

const UPGRADE_SESSION: SessionIndex = 3;

const UPGRADE_FINALIZATION_WAIT_SESSIONS: u32 = 3;

// Simple test that schedules a version upgrade, awaits it, and checks if node is still finalizing after planned upgrade session.
pub async fn schedule_version_change(config: &Config) -> anyhow::Result<()> {
    let connection = config.create_root_connection().await;
    let test_case_params = config.test_case_params.clone();

    let current_session = connection.connection.get_session(None).await;
    let version_for_upgrade = test_case_params
        .upgrade_to_version
        .unwrap_or(UPGRADE_TO_VERSION);
    let session_for_upgrade =
        current_session + test_case_params.upgrade_session.unwrap_or(UPGRADE_SESSION);
    let wait_sessions_after_upgrade = test_case_params
        .upgrade_finalization_wait_sessions
        .unwrap_or(UPGRADE_FINALIZATION_WAIT_SESSIONS);
    let session_after_upgrade = session_for_upgrade + wait_sessions_after_upgrade;

    connection
        .schedule_finality_version_change(
            version_for_upgrade,
            session_for_upgrade,
            TxStatus::Finalized,
        )
        .await?;
    connection
        .connection
        .wait_for_session(session_after_upgrade + 1, BlockStatus::Finalized)
        .await;

    let block_number = connection.connection.get_best_block().await;
    connection
        .connection
        .wait_for_block(|n| n >= block_number, BlockStatus::Finalized)
        .await;

    Ok(())
}
