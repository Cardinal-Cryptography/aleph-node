use reqwest::Client;
use serde::{Deserialize, Serialize};

// This code is a copy-paste of data structures declared in the `synthetic-network/rush` project.
#[derive(Serialize, Deserialize)]
pub struct SyntheticNetwork {
    default_link: SyntheticLink,
    flows: Vec<SyntheticFlow>,
}

#[derive(Serialize, Deserialize)]
pub struct SyntheticLink {
    ingress: QoS,
    egress: QoS,
}

#[derive(Serialize, Deserialize)]
pub struct QoS {
    rate: u64,
    loss: f64,
    latency: u64,
    jitter: u64,
    jitter_strength: f64,
    reorder_packets: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SyntheticFlow {
    label: String,
    flow: Flow,
    link: SyntheticLink,
}

#[derive(Serialize, Deserialize)]
pub struct Flow {
    ip: u32,
    protocol: u8,
    port_min: u16,
    port_max: u16,
}

pub struct SyntheticNetworkClient {
    client: Client,
    url: String,
}

pub struct NetworkConfig {
    config: SyntheticNetwork,
}

const DEFAULT_QOS: QoS = QoS {
    rate: 1000000000,
    loss: 0.0,
    latency: 0,
    jitter: 0,
    jitter_strength: 0.0,
    reorder_packets: false,
};

impl NetworkConfig {
    pub fn new() -> Self {
        NetworkConfig {
            config: SyntheticNetwork {
                default_link: SyntheticLink {
                    ingress: DEFAULT_QOS,
                    egress: DEFAULT_QOS,
                },
                flows: Vec::default(),
            },
        }
    }

    pub fn set_out_latency(&mut self, latency_milliseconds: u64) -> &mut Self {
        self.config.default_link.egress.latency = latency_milliseconds;
        self
    }

    pub fn set_in_latency(&mut self, latency_milliseconds: u64) -> &mut Self {
        self.config.default_link.ingress.latency = latency_milliseconds;
        self
    }

    pub fn set_out_rate(&mut self, rate: u64) -> &mut Self {
        self.config.default_link.egress.rate = rate;
        self
    }

    pub fn set_in_rate(&mut self, rate: u64) -> &mut Self {
        self.config.default_link.ingress.rate = rate;
        self
    }

    pub fn into_synthetic_network(&self) -> &SyntheticNetwork {
        &self.config
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

impl SyntheticNetworkClient {
    pub fn new_url(url: impl Into<String>) -> Self {
        SyntheticNetworkClient {
            client: Client::new(),
            url: url.into(),
        }
    }

    pub async fn commit_config(&mut self, config: &NetworkConfig) -> anyhow::Result<()> {
        let result = self
            .client
            .post(&self.url)
            .json(config.into_synthetic_network())
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
