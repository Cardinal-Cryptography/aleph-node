use futures::future::join_all;

use crate::{
    config::setup_test,
    synthetic_network::{set_out_latency, wait_for_further_finalized_blocks},
};

const OUT_LATENCY: u64 = 200;

/// Test if nodes are able to proceed despite high latency. More precisely, it first awaits predefined number of blocks, sets
/// egress-latency for each node using same value (default is 200 milliseconds) and verifies if after it was able to proceed
/// twice as much blocks on high latency
#[tokio::test]
pub async fn high_out_latency_for_all() -> anyhow::Result<()> {
    let config = setup_test();
    let out_latency = config.test_case_params.out_latency.unwrap_or(OUT_LATENCY);
    let blocks_to_wait: u32 = 30;

    let connections = config.create_signed_connections().await;
    join_all(
        connections
            .iter()
            .map(|connection| wait_for_further_finalized_blocks(connection, blocks_to_wait)),
    )
    .await;
    join_all(
        config
            .synthetic_network_urls()
            .into_iter()
            .map(|synthetic_url| set_out_latency(out_latency, synthetic_url)),
    )
    .await;
    join_all(
        connections
            .iter()
            .map(|connection| wait_for_further_finalized_blocks(connection, blocks_to_wait * 2)),
    )
    .await;
    Ok(())
}

/// Test if nodes are able to proceed despite high latency. More precisely, it first awaits predefined number of blocks, sets
/// egress-latency for 1/3 of nodes using same value (default is 200 milliseconds) and verifies if after it was able to proceed
/// twice as much blocks on high latency
#[tokio::test]
pub async fn high_out_latency_for_each_quorum() -> anyhow::Result<()> {
    let config = setup_test();
    let out_latency = config.test_case_params.out_latency.unwrap_or(OUT_LATENCY);
    let blocks_to_wait: u32 = 30;

    let connections = config.create_signed_connections().await;
    join_all(
        connections
            .iter()
            .map(|connection| wait_for_further_finalized_blocks(connection, blocks_to_wait)),
    )
    .await;
    join_all(
        config
            .synthetic_network_urls()
            .into_iter()
            .take(((config.validator_count - 1) / 3 + 1) as usize)
            .map(|synthetic_url| set_out_latency(out_latency, synthetic_url)),
    )
    .await;
    join_all(
        connections
            .iter()
            .map(|connection| wait_for_further_finalized_blocks(connection, blocks_to_wait * 2)),
    )
    .await;

    Ok(())
}
