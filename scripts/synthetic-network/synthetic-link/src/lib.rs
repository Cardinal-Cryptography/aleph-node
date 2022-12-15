use std::ops::RangeInclusive;

use reqwest::Client;
use serde::{Deserialize, Serialize};

// This section is an exact copy of data structures declared in the `synthetic-network/rush` project.

#[derive(Serialize, Deserialize)]
struct SyntheticNetwork {
    default_link: SyntheticLink,
    flows: Vec<SyntheticFlow>,
}

#[derive(Serialize, Deserialize)]
struct SyntheticLink {
    ingress: QoS,
    egress: QoS,
}

#[derive(Serialize, Deserialize)]
struct QoS {
    rate: u64,
    loss: f64,
    latency: u64,
    jitter: u64,
    jitter_strength: f64,
    reorder_packets: bool,
}

#[derive(Serialize, Deserialize)]
struct SyntheticFlow {
    label: String,
    flow: Flow,
    link: SyntheticLink,
}

#[derive(Serialize, Deserialize)]
struct Flow {
    ip: u32,
    protocol: u8,
    port_min: u16,
    port_max: u16,
}

// end of copy-paste

#[derive(Serialize, Deserialize)]
pub struct SyntheticNetworkJson(SyntheticNetwork);

pub struct SyntheticNetworkClient {
    client: Client,
    url: String,
}

const DEFAULT_QOS: QoS = QoS {
    rate: 1000000000,
    loss: 0.0,
    latency: 0,
    jitter: 0,
    jitter_strength: 0.0,
    reorder_packets: false,
};

const DEFAULT_FLOW: Flow = Flow {
    ip: 0,
    protocol: 0,
    port_min: 0,
    port_max: 0,
};

const DEFAULT_LINK: SyntheticLink = SyntheticLink {
    ingress: DEFAULT_QOS,
    egress: DEFAULT_QOS,
};

const DEFAULT_NAMED_FLOW: SyntheticFlow = SyntheticFlow {
    label: String::new(),
    flow: DEFAULT_FLOW,
    link: DEFAULT_LINK,
};

pub struct NetworkConfig {
    config: SyntheticNetwork,
}

impl NetworkConfig {
    pub const fn new() -> Self {
        NetworkConfig {
            config: SyntheticNetwork {
                default_link: SyntheticLink {
                    ingress: DEFAULT_QOS,
                    egress: DEFAULT_QOS,
                },
                flows: Vec::new(),
            },
        }
    }

    pub fn ingress(&mut self, qos: NetworkQoS) -> &mut Self {
        self.config.default_link.ingress = qos.qos;
        self
    }
    pub fn egress(&mut self, qos: NetworkQoS) -> &mut Self {
        self.config.default_link.egress = qos.qos;
        self
    }

    pub fn add_flow(&mut self, flow: NetworkFlow) -> &mut Self {
        self.config.flows.push(flow.flow);
        self
    }

    fn into_synthetic_network(&self) -> &SyntheticNetwork {
        &self.config
    }
}

pub struct NetworkFlow {
    flow: SyntheticFlow,
}

impl NetworkFlow {
    pub const fn new() -> Self {
        NetworkFlow {
            flow: DEFAULT_NAMED_FLOW,
        }
    }

    pub fn label(&mut self, label: String) -> &mut Self {
        self.flow.label = label;
        self
    }

    pub fn ip(&mut self, ip: IpPattern) -> &mut Self {
        let ip = match ip {
            IpPattern::All => 0,
            IpPattern::Ip(ip) => ip,
        };
        self.flow.flow.ip = ip;
        self
    }

    pub fn protocol(&mut self, protocol: Protocol) -> &mut Self {
        let protocol_id = match protocol {
            Protocol::Icmp => 1,
            Protocol::Tcp => 6,
            Protocol::Udp => 17,
            Protocol::All => 0,
        };
        self.flow.flow.protocol = protocol_id;
        self
    }

    pub fn port_range(&mut self, port_range: RangeInclusive<u16>) -> &mut Self {
        self.flow.flow.port_min = *port_range.start();
        self.flow.flow.port_max = *port_range.end();
        self
    }
}

pub enum IpPattern {
    All,
    Ip(u32),
}

pub enum Protocol {
    Icmp,
    Tcp,
    Udp,
    All,
}

pub struct NetworkLink {
    link: SyntheticLink,
}

impl NetworkLink {
    pub const fn new() -> Self {
        NetworkLink { link: DEFAULT_LINK }
    }

    pub fn ingress(&mut self, qos: NetworkQoS) -> &mut Self {
        self.link.ingress = qos.qos;
        self
    }

    pub fn egress(&mut self, qos: NetworkQoS) -> &mut Self {
        self.link.egress = qos.qos;
        self
    }
}

pub struct NetworkQoS {
    qos: QoS,
}

impl NetworkQoS {
    pub const fn new() -> Self {
        NetworkQoS { qos: DEFAULT_QOS }
    }

    pub fn latency(&mut self, latency_milliseconds: u64) -> &mut Self {
        self.qos.latency = latency_milliseconds;
        self
    }

    pub fn rate(&mut self, rate: u64) -> &mut Self {
        self.qos.rate = rate;
        self
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig::new()
    }
}

impl From<SyntheticNetwork> for NetworkConfig {
    fn from(synth_net: SyntheticNetwork) -> Self {
        Self { config: synth_net }
    }
}

impl From<SyntheticNetworkJson> for NetworkConfig {
    fn from(value: SyntheticNetworkJson) -> Self {
        NetworkConfig::from(value.0)
    }
}

impl SyntheticNetworkClient {
    pub fn new(url: String) -> Self {
        SyntheticNetworkClient {
            client: Client::new(),
            url,
        }
    }

    pub async fn commit_config(&mut self, config: &NetworkConfig) -> anyhow::Result<()> {
        let result = self
            .client
            .post(&self.url)
            .json(&config.into_synthetic_network())
            .send()
            .await;
        Ok(result.map(|_| ())?)
    }

    pub async fn load_config(&mut self) -> anyhow::Result<NetworkConfig> {
        let result = self
            .client
            .get(&self.url)
            .send()
            .await?
            .json::<SyntheticNetwork>()
            .await;
        Ok(result.map(NetworkConfig::from)?)
    }
}
