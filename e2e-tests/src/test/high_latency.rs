use std::cmp::max;

use aleph_client::{
    pallets::session::SessionApi,
    waiting::{AlephWaiting, BlockStatus},
};
use log::info;

use crate::{config::setup_test, synthetic_network::set_out_latency};

#[tokio::test]
pub async fn high_out_latency() -> anyhow::Result<()> {
    let config = setup_test();

    let connections = config.create_signed_connections().await;
    info!("waiting for at least session 3");
    for connection in &connections {
        if connection.get_session(None).await < 3 {
            connection.wait_for_session(3, BlockStatus::Finalized).await;
        }
    }

    info!("setting out-latency");
    for validator in config.validator_names() {
        info!(
            "setting out-latency of node {} to {}",
            validator, config.test_case_params.out_latency
        );
        set_out_latency(config.test_case_params.out_latency, &validator).await;
    }

    let mut max_session = 0;
    for connection in &connections {
        let node_session = connection.get_session(None).await;
        max_session = max(max_session, node_session);
    }
    info!("current session is {}", max_session);

    for connection in connections {
        connection
            .wait_for_session(max_session + 2, BlockStatus::Finalized)
            .await;
    }
    Ok(())
}

#[tokio::test]
pub async fn no_quorum_without_high_out_latency() -> anyhow::Result<()> {
    let config = setup_test();

    let connections = config.create_signed_connections().await;
    info!("waiting for at least session 3");
    for connection in &connections {
        if connection.get_session(None).await < 3 {
            connection.wait_for_session(3, BlockStatus::Finalized).await;
        }
    }

    info!("setting out-latency");
    for validator in config
        .validator_names()
        .into_iter()
        .take(((config.validator_count - 1) / 3 + 1) as usize)
    {
        info!(
            "setting out-latency of node {} to {}",
            validator, config.test_case_params.out_latency
        );
        set_out_latency(config.test_case_params.out_latency, &validator).await;
    }

    let mut max_session = 0;
    for connection in &connections {
        let node_session = connection.get_session(None).await;
        max_session = max(max_session, node_session);
    }
    info!("current session is {}", max_session);

    info!("waiting for session {}", max_session + 2);
    for connection in connections {
        connection
            .wait_for_session(max_session + 2, BlockStatus::Finalized)
            .await;
    }
    Ok(())
}
