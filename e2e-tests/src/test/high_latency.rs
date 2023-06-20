use crate::{config::setup_test, synthetic_network::test_latency_template_test};

/// Test if nodes are able to proceed despite high latency. More precisely, it first awaits predefined number of blocks, sets
/// egress-latency for each node using same value (default is 200 milliseconds) and verifies if after it was able to proceed
/// twice as much blocks on high latency
#[tokio::test]
pub async fn high_out_latency_for_all() -> anyhow::Result<()> {
    let config = setup_test();
    test_latency_template_test(&config, config.validator_count as usize).await?;

    Ok(())
}

/// Test if nodes are able to proceed despite high latency. More precisely, it first awaits predefined number of blocks, sets
/// egress-latency for 1/3 of nodes using same value (default is 200 milliseconds) and verifies if after it was able to proceed
/// twice as much blocks on high latency
#[tokio::test]
pub async fn high_out_latency_for_each_quorum() -> anyhow::Result<()> {
    let config = setup_test();
    test_latency_template_test(&config, ((config.validator_count - 1) / 3 + 1) as usize).await?;

    Ok(())
}
