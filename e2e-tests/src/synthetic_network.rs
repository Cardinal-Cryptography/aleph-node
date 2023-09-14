use std::{
    cmp::max,
    collections::{hash_map::Entry, HashMap},
    mem::swap,
    net::Ipv4Addr,
};

use aleph_client::{
    utility::BlocksApi,
    waiting::{AlephWaiting, BlockStatus},
    SignedConnection,
};
use anyhow::{anyhow, Context};
use futures::{
    future::{join_all, try_join_all},
    Future,
};
use log::info;
use synthetic_link::{
    IpPattern, NonEmptyString, PortRange, Protocol, QualityOfService, SyntheticFlow,
    SyntheticNetwork, SyntheticNetworkClient,
};
use url::Url;

use crate::config::{Config, NodeConfig};

pub type Milliseconds = u64;

pub const OUT_LATENCY: Milliseconds = 200;

pub struct SyntheticNetworkConfigurator {
    config: SyntheticNetwork,
}

impl SyntheticNetworkConfigurator {
    pub fn new(config: SyntheticNetwork) -> Self {
        Self { config }
    }

    pub async fn retrieve_from_host(synthetic_network_endpoint_url: Url) -> anyhow::Result<Self> {
        let mut client = SyntheticNetworkClient::new(synthetic_network_endpoint_url.to_string());
        let config = client.load_config().await?;
        Ok(Self { config })
    }

    pub fn set_out_latency(&mut self, latency: Milliseconds) -> &mut Self {
        self.config.default_link.egress.latency = latency;
        self
    }

    fn set_rate_configuration(&mut self, rate: u64, node: Ipv4Addr) -> &mut Self {
        let node_int: u32 = node.into();
        let node_int = node_int.to_be();
        let label = format!("{}", node_int);

        info!(
            "creating a synthetic-network flow with label {} for node {} with bit-rate of {}",
            &label, &node, rate
        );

        let flow = self
            .config
            .flows
            .iter_mut()
            .find(|flow| flow.label.as_ref().to_owned() == label);
        let flow = if let Some(flow) = flow {
            flow
        } else {
            let flow =
                SyntheticFlow::new(NonEmptyString::new(label).expect("provided non-empty label"));
            self.config.flows.push(flow);
            self.config
                .flows
                .last_mut()
                .expect("should be able to get last element of a non-empty Vec")
        };
        flow.flow.ip = IpPattern::Ip(node_int);
        flow.flow.protocol = Protocol::All;
        flow.flow.port_range = PortRange::all();
        flow.link.ingress.rate = rate;
        flow.link.egress.rate = rate;
        self
    }

    pub fn disconnect_node_from(&mut self, nodes: impl IntoIterator<Item = Ipv4Addr>) -> &mut Self {
        for node in nodes {
            self.set_rate_configuration(0, node);
        }
        self
    }

    pub fn connect_node_to(&mut self, nodes: impl IntoIterator<Item = Ipv4Addr>) -> &mut Self {
        for node in nodes {
            self.set_rate_configuration(QualityOfService::default().rate, node);
        }
        self
    }

    pub async fn commit_config(
        &mut self,
        client: &mut SyntheticNetworkClient,
    ) -> anyhow::Result<()> {
        client.commit_config(&self.config).await
    }
}

impl AsRef<SyntheticNetwork> for SyntheticNetworkConfigurator {
    fn as_ref(&self) -> &SyntheticNetwork {
        &self.config
    }
}

impl From<SyntheticNetworkConfigurator> for SyntheticNetwork {
    fn from(value: SyntheticNetworkConfigurator) -> Self {
        value.config
    }
}

pub async fn set_out_latency(
    milliseconds: Milliseconds,
    synthetic_url: String,
) -> anyhow::Result<()> {
    info!(
        "setting out-latency of node {} to {}ms",
        synthetic_url, milliseconds
    );
    let mut client = SyntheticNetworkClient::new(synthetic_url);
    let config = client.load_config().await?;
    let mut config = SyntheticNetworkConfigurator::new(config);
    config
        .set_out_latency(milliseconds)
        .commit_config(&mut client)
        .await
        .context("unable to commit network configuration")
}

#[derive(Clone)]
pub struct ConnectivityConfiguration {
    nodes: Vec<NodeConfig>,
    to_connect: Vec<NodeConfig>,
    to_disconnect: Vec<NodeConfig>,
}

impl ConnectivityConfiguration {
    pub fn reconnect(&mut self) -> &mut Self {
        swap(&mut self.to_connect, &mut self.to_disconnect);
        self.to_disconnect = vec![];
        self
    }
}

#[derive(Clone)]
pub struct NodesConnectivityConfiguration(Vec<ConnectivityConfiguration>);

type GroupedNodes = Vec<Vec<NodeConfig>>;

impl From<GroupedNodes> for NodesConnectivityConfiguration {
    fn from(groups: Vec<Vec<NodeConfig>>) -> Self {
        let mut grouped = Vec::with_capacity(groups.len());
        for (group_index, group) in groups.iter().enumerate() {
            let other_nodes = groups
                .iter()
                .enumerate()
                .filter_map(|(index, group)| (index != group_index).then_some(group.iter()))
                .flatten()
                .cloned()
                .collect();

            grouped.push(ConnectivityConfiguration {
                nodes: group.to_vec(),
                to_connect: vec![],
                to_disconnect: other_nodes,
            });
        }
        Self(grouped)
    }
}

impl IntoIterator for NodesConnectivityConfiguration {
    type Item = ConnectivityConfiguration;

    type IntoIter = <Vec<ConnectivityConfiguration> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl NodesConnectivityConfiguration {
    pub async fn commit(&self) -> anyhow::Result<()> {
        let mut configs = HashMap::new();
        for config in &self.0 {
            for node in &config.nodes {
                info!(
                    "Building connectivity configuration for node {}",
                    node.node_name()
                );
                let node_entry = configs.entry(node.synthetic_network_url().to_string());
                let update = |configurator: &mut SyntheticNetworkConfigurator| {
                    configurator.connect_node_to(
                        config
                            .to_connect
                            .iter()
                            .map(|node| node.ip_address())
                            .cloned(),
                    );
                    configurator.disconnect_node_from(
                        config
                            .to_disconnect
                            .iter()
                            .map(|node| node.ip_address())
                            .cloned(),
                    );
                };
                let node_entry = node_entry.and_modify(|(configurator, _)| update(configurator));

                if let Entry::Vacant(entry) = node_entry {
                    let mut client =
                        SyntheticNetworkClient::new(node.synthetic_network_url().to_string());
                    let config = client.load_config().await?;
                    let mut configurator = SyntheticNetworkConfigurator::new(config);
                    update(&mut configurator);
                    entry.insert((configurator, client));
                }
            }
        }

        for (mut configurator, mut client) in configs.into_values() {
            configurator.commit_config(&mut client).await?;
        }

        Ok(())
    }

    pub fn merge(mut self, mut config: NodesConnectivityConfiguration) -> Self {
        self.0.append(&mut config.0);
        self
    }

    pub fn reconnect(mut self) -> Self {
        for configuration in &mut self.0 {
            configuration.reconnect();
        }
        self
    }
}

async fn wait_for_further_finalized_blocks(
    connection: &SignedConnection,
    blocks_to_wait: u32,
) -> anyhow::Result<()> {
    let finalized = connection.get_finalized_block_hash().await?;
    let finalized_number = connection
        .get_block_number(finalized)
        .await?
        .ok_or(anyhow!(
            "Failed to retrieve block number for hash {finalized:?}"
        ))?;
    let block_number_to_wait = finalized_number + blocks_to_wait;
    info!(
        "Current finalized block #{}, waiting for block #{}",
        finalized_number, block_number_to_wait
    );
    connection
        .wait_for_block(|n| n > block_number_to_wait, BlockStatus::Finalized)
        .await;
    Ok(())
}

pub async fn await_new_blocks<'a>(
    nodes: impl IntoIterator<Item = &'a NodeConfig>,
    blocks_to_wait: u32,
) -> anyhow::Result<u32> {
    info!("Awaiting new blocks");

    let nodes = join_all(
        nodes
            .into_iter()
            .map(|config| async { (config.node_name(), config.create_signed_connection().await) }),
    )
        .await;

    let mut best_seen_block = 0;
    for (node_name, connection) in nodes.iter() {
        let best_block = connection.get_best_block().await?.unwrap_or(0);
        info!("Best block for {} is {}", node_name, best_block);
        best_seen_block = max(best_seen_block, best_block);
    }
    for (node_name, connection) in nodes.iter() {
        let wait_for_block = best_seen_block + blocks_to_wait;
        info!("Waiting for {} at {}", wait_for_block, node_name);
        connection
            .wait_for_block(|block| block >= wait_for_block, BlockStatus::Best)
            .await;
    }
    Ok(best_seen_block)
}

pub async fn await_finalized_blocks<'a>(
    nodes: impl IntoIterator<Item = &'a NodeConfig>,
    mut best_seen_block: u32,
    blocks_to_wait: u32,
) -> anyhow::Result<()> {
    info!("Checking finalization");

    let nodes = join_all(
        nodes
            .into_iter()
            .map(|config| async { (config.node_name(), config.create_signed_connection().await) }),
    )
    .await;

    for (node_name, connection) in nodes.iter() {
        let finalized = connection.get_finalized_block_hash().await?;
        let finalized_number =
            connection
                .get_block_number(finalized)
                .await?
                .ok_or(anyhow::anyhow!(
                    "Failed to retrieve block number for hash {finalized:?} at node {node_name}"
                ))?;
        best_seen_block = max(best_seen_block, finalized_number);
    }
    let wait_block = best_seen_block + blocks_to_wait;

    for (node_name, connection) in nodes.iter() {
        info!("Awaiting finalization of the block {wait_block} at node {node_name}",);
        connection
            .wait_for_block(|n| n > wait_block, BlockStatus::Finalized)
            .await;
    }
    Ok(())
}

pub async fn test_latency_template_test(
    config: &Config,
    validator_count: usize,
    out_latency: Milliseconds,
) -> anyhow::Result<()> {
    let connections = config.create_signed_connections().await;
    join_all(
        config
            .synthetic_network_urls()
            .into_iter()
            .take(validator_count)
            .map(|synthetic_url| set_out_latency(out_latency, synthetic_url)),
    )
    .await;
    info!("Waiting for session 1");
    join_all(
        connections
            .iter()
            .map(|connection| connection.wait_for_session(1, BlockStatus::Finalized)),
    )
    .await;
    let blocks_to_wait_in_first_session = 30;
    info!(
        "Waiting for {} finalized blocks in sesssion 1 to make sure initial unit collection works.",
        blocks_to_wait_in_first_session
    );
    try_join_all(connections.iter().map(|connection| {
        wait_for_further_finalized_blocks(connection, blocks_to_wait_in_first_session)
    }))
    .await?;
    Ok(())
}

pub async fn execute_synthetic_network_test(
    nodes_under_test: impl IntoIterator<Item = &NodeConfig>,
    action: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    let mut configs = Vec::new();
    for config in nodes_under_test {
        let mut client = SyntheticNetworkClient::new(config.synthetic_network_url().to_string());
        let config = client.load_config().await?;
        configs.push((client, config));
    }

    action.await?;

    for (mut client, config) in configs {
        client.commit_config(&config).await?;
    }

    Ok(())
}
