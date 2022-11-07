use aleph_client::{
    get_block_hash, get_next_session_finality_version, get_session_finality_version, ReadStorage,
};
use log::info;
use primitives::{BlockNumber, Version};

pub fn check_finality_version_at_block<C: ReadStorage>(
    connection: &C,
    block_number: BlockNumber,
    expected_version: Version,
) {
    info!(
        "Checking current session finality version for block {}",
        block_number
    );
    let block_hash = get_block_hash(connection, block_number);
    let finality_version = get_session_finality_version(connection, Some(block_hash));
    assert_eq!(finality_version, expected_version);
}

pub fn check_next_session_finality_version_at_block<C: ReadStorage>(
    connection: &C,
    block_number: BlockNumber,
    expected_version: Version,
) {
    info!(
        "Checking next session finality version for block {}",
        block_number
    );
    let block_hash = get_block_hash(connection, block_number);
    let next_finality_version = get_next_session_finality_version(connection, Some(block_hash));
    assert_eq!(next_finality_version, expected_version);
}
