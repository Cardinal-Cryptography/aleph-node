use aleph_client::{
    get_session_period, schedule_finality_version_change, wait_for_finalized_block, XtStatus,
};
use log::info;
use primitives::{BlockNumber, SessionIndex, Version, DEFAULT_FINALITY_VERSION};

use crate::{
    finality_version::{
        check_finality_version_at_block, check_next_session_finality_version_at_block,
    },
    Config,
};

const FIRST_INCOMING_FINALITY_VERSION: Version = 1;
const SESSION_WITH_FINALITY_VERSION_CHANGE: SessionIndex = 4;
const SCHEDULING_OFFSET_SESSIONS: f64 = -2.5;
const CHECK_START_BLOCK: BlockNumber = 0;

/// Sets up the test. Waits for block 2.5 sessions ahead of `SESSION_WITH_FINALITY_VERSION_CHANGE`.
/// Schedules a finality version change. Waits for all blocks of session
/// `SESSION_WITH_FINALITY_VERSION_CHANGE` to be finalized. Checks the finality version and the
/// finality version for the next session for all the blocks from block `CHECK_START_BLOCK`
/// until the end of session `SESSION_WITH_FINALITY_VERSION_CHANGE`.
pub fn finality_version(config: &Config) -> anyhow::Result<()> {
    let root_connection = config.create_root_connection();
    let session_period = get_session_period(&root_connection);

    let start_point_in_sessions =
        SESSION_WITH_FINALITY_VERSION_CHANGE as f64 + SCHEDULING_OFFSET_SESSIONS;
    let scheduling_block = (start_point_in_sessions * session_period as f64) as u32;
    let end_block = (SESSION_WITH_FINALITY_VERSION_CHANGE + 1) * session_period - 1;

    info!(
        "Finality version check | start block: {} | end block: {}",
        CHECK_START_BLOCK, end_block,
    );
    info!(
        "Version change to be scheduled on block {} for block {}",
        scheduling_block,
        SESSION_WITH_FINALITY_VERSION_CHANGE * session_period
    );
    wait_for_finalized_block(&root_connection, scheduling_block)?;

    schedule_finality_version_change(
        &root_connection,
        FIRST_INCOMING_FINALITY_VERSION,
        SESSION_WITH_FINALITY_VERSION_CHANGE,
        XtStatus::Finalized,
    );

    wait_for_finalized_block(&root_connection, end_block)?;

    let finality_change_block = SESSION_WITH_FINALITY_VERSION_CHANGE * session_period;
    let last_block_with_default_next_session_finality_version =
        finality_change_block - session_period - 1;

    info!(
        "Checking default finality versions. Blocks {} to {}",
        CHECK_START_BLOCK, last_block_with_default_next_session_finality_version
    );
    for block in CHECK_START_BLOCK..(last_block_with_default_next_session_finality_version + 1) {
        check_finality_version_at_block(&root_connection, block, DEFAULT_FINALITY_VERSION);
        check_next_session_finality_version_at_block(
            &root_connection,
            block,
            DEFAULT_FINALITY_VERSION,
        );
    }

    info!(
        "Checking finality versions for session prior to the change. Blocks {} to {}",
        last_block_with_default_next_session_finality_version + 1,
        finality_change_block - 1
    );
    for block in (last_block_with_default_next_session_finality_version + 1)..finality_change_block
    {
        check_finality_version_at_block(&root_connection, block, DEFAULT_FINALITY_VERSION);
        check_next_session_finality_version_at_block(
            &root_connection,
            block,
            FIRST_INCOMING_FINALITY_VERSION,
        );
    }
    info!(
        "Checking finality versions once the change has come into effect. Blocks {} to {}",
        finality_change_block, end_block
    );
    for block in finality_change_block..(end_block + 1) {
        check_finality_version_at_block(&root_connection, block, FIRST_INCOMING_FINALITY_VERSION);
        check_next_session_finality_version_at_block(
            &root_connection,
            block,
            FIRST_INCOMING_FINALITY_VERSION,
        );
    }

    Ok(())
}
