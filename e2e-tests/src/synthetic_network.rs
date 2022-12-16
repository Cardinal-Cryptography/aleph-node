use log::info;
use synthetic_link::SyntheticNetworkClient;

pub type Milliseconds = u64;

fn create_client(node_name: &str) -> SyntheticNetworkClient {
    let synthetic_network_url = format!("http://{}:80/qos", node_name);
    info!("creating an http client for url {}", synthetic_network_url);
    SyntheticNetworkClient::new(synthetic_network_url)
}

pub async fn set_out_latency(milliseconds: Milliseconds, node_name: &str) {
    info!(
        "setting out-latency of node {} to {}ms",
        node_name, milliseconds
    );
    let mut client = create_client(node_name);
    let mut config = client
        .load_config()
        .await
        .expect("we should be able to download config of the synthetic-network ");

    config.default_link.egress.latency = milliseconds;

    client
        .commit_config(&config)
        .await
        .expect("unable to commit network configuration");
}
