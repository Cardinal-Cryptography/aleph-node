use aleph_client::RootConnection;
use anyhow::Error;
use primitives::SessionIndex;

use crate::commands::{ExtrinsicState, Version};

pub fn schedule_upgrade(
    connection: RootConnection,
    version: Version,
    session_for_upgrade: SessionIndex,
    expected_state: ExtrinsicState,
) -> anyhow::Result<()> {
    aleph_client::schedule_upgrade(
        &connection,
        version,
        session_for_upgrade,
        expected_state.into(),
    )
    .map_err(Error::new)
}
