use aleph_client::{
    get_current_block_number, get_current_finality_version, get_current_session,
    get_next_session_finality_version, schedule_finality_version_change, wait_for_finalized_block,
    wait_for_session, AnyConnection, XtStatus,
};
use log::info;
use primitives::{Version, DEFAULT_FINALITY_VERSION, DEFAULT_SESSION_PERIOD};

use crate::Config;

const FIRST_INCOMING_FINALITY_VERSION: Version = 1;

pub fn check_finality_version_for_all_blocks_in_current_session<C: AnyConnection>(
    connection: &C,
    expected_finality_version: Version,
    expected_finality_version_next_session: Version,
) -> anyhow::Result<()> {
    let mut current_block_number = get_current_block_number(connection);
    let current_session = get_current_session(connection);

    while current_block_number < (current_session + 1) * DEFAULT_SESSION_PERIOD - 1 {
        info!("Checking finality versions for block number {:?}", current_block_number + 1);
        current_block_number = wait_for_finalized_block(connection, current_block_number + 1)?;

        let current_finality_version = get_current_finality_version(connection);
        assert_eq!(current_finality_version, expected_finality_version);

        let next_session_finality_version = get_next_session_finality_version(connection);
        assert_eq!(
            next_session_finality_version,
            expected_finality_version_next_session
        );
    }

    Ok(())
}

pub fn finality_version(config: &Config) -> anyhow::Result<()> {
    let root_connection = config.create_root_connection();

    let start_session = get_current_session(&root_connection);
    info!("Start session: {}", start_session);

    wait_for_session(&root_connection, start_session + 2)?;

    let current_session = get_current_session(&root_connection);
    info!("Current session: {}", current_session);

    info!("Checking finality versions with no version change ever scheduled");
    let current_finality_version = get_current_finality_version(&root_connection);
    assert_eq!(current_finality_version, DEFAULT_FINALITY_VERSION);
    let next_session_finality_version = get_next_session_finality_version(&root_connection);
    assert_eq!(next_session_finality_version, DEFAULT_FINALITY_VERSION);

    let current_session = get_current_session(&root_connection);
    info!("Current session: {:?}", current_session);

    let session_before_version_change = current_session + 1;
    let session_with_version_change = current_session + 2;
    let session_after_version_change = current_session + 3;

    info!(
        "Scheduling finality version change | version {} incoming on session {}",
        FIRST_INCOMING_FINALITY_VERSION, session_with_version_change
    );
    schedule_finality_version_change(
        &root_connection,
        FIRST_INCOMING_FINALITY_VERSION,
        session_with_version_change,
        XtStatus::Finalized,
    );

    wait_for_session(&root_connection, session_before_version_change)?;

    let current_session = get_current_session(&root_connection);

    // Check finality versions for all blocks of sessions k - 1, k, k + 1, where k is the session
    // with the scheduled version change.
    info!(
        "Checking finality versions before scheduled version change | session {}",
        current_session
    );
    check_finality_version_for_all_blocks_in_current_session(
        &root_connection,
        DEFAULT_FINALITY_VERSION,
        FIRST_INCOMING_FINALITY_VERSION,
    )?;

    wait_for_session(&root_connection, session_with_version_change)?;
    let current_session = get_current_session(&root_connection);

    info!(
        "Checking finality versions on scheduled version change | session {}",
        current_session
    );
    check_finality_version_for_all_blocks_in_current_session(
        &root_connection,
        FIRST_INCOMING_FINALITY_VERSION,
        FIRST_INCOMING_FINALITY_VERSION,
    )?;

    wait_for_session(&root_connection, session_after_version_change)?;
    let current_session = get_current_session(&root_connection);

    info!(
        "Checking finality versions after scheduled version change | session {}",
        current_session
    );
    check_finality_version_for_all_blocks_in_current_session(
        &root_connection,
        FIRST_INCOMING_FINALITY_VERSION,
        FIRST_INCOMING_FINALITY_VERSION,
    )?;

    Ok(())
}
