use log::info;
use synthetic_link::{NetworkLink, NetworkQoS, SyntheticNetworkClient};

pub type Milliseconds = u64;

fn create_client(node_name: impl AsRef<str>) -> SyntheticNetworkClient {
    let synthetic_network_url = format!("http://{}:80/qos", node_name.as_ref());
    info!("creating an http client for url {}", synthetic_network_url);
    SyntheticNetworkClient::new(synthetic_network_url)
}

pub async fn set_out_latency(milliseconds: Milliseconds, node_name: impl AsRef<str>) {
    info!(
        "setting out-latency of node {} to {}ms",
        node_name.as_ref(),
        milliseconds
    );
    let mut client = create_client(node_name);
    let mut config = client
        .load_config()
        .await
        .expect("we should be able to download synthetic-network's config");

    let mut network_qos = NetworkQoS::default();
    network_qos.latency(milliseconds);
    let mut network_link = NetworkLink::default();
    network_link.egress(network_qos);
    config.link(network_link);

    client
        .commit_config(&config)
        .await
        .expect("unable to commit network configuration");
}
