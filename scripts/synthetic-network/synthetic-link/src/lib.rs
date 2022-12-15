use std::{borrow::Borrow, ops::RangeInclusive};

use anyhow::bail;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

const DEFAULT_SYNTHETIC_NETWORK: SyntheticNetwork = SyntheticNetwork {
    default_link: DEFAULT_SYNTHETIC_LINK,
    flows: Vec::new(),
};

const DEFAULT_SYNTHETIC_LINK: SyntheticLink = SyntheticLink {
    ingress: DEFAULT_QOS,
    egress: DEFAULT_QOS,
};

const DEFAULT_QOS: QoS = QoS {
    rate: 1000000000,
    loss: 0.0,
    latency: 0,
    jitter: 0,
    jitter_strength: 0.0,
    reorder_packets: false,
};

const DEFAULT_SYNTHETIC_FLOW: SyntheticFlow = SyntheticFlow {
    label: String::new(),
    flow: DEFAULT_FLOW,
    link: DEFAULT_SYNTHETIC_LINK,
};

const DEFAULT_FLOW: Flow = Flow {
    ip: IpPattern::All,
    protocol: Protocol::All,
    port_range: PortRange(0..=0),
};

#[derive(Serialize, Deserialize, Clone)]
pub struct SyntheticNetwork {
    pub default_link: SyntheticLink,
    pub flows: Vec<SyntheticFlow>,
}

impl Default for SyntheticNetwork {
    fn default() -> Self {
        DEFAULT_SYNTHETIC_NETWORK
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SyntheticLink {
    pub ingress: QoS,
    pub egress: QoS,
}

impl Default for SyntheticLink {
    fn default() -> Self {
        DEFAULT_SYNTHETIC_LINK
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct QoS {
    pub rate: u64,
    pub loss: f64,
    pub latency: u64,
    pub jitter: u64,
    pub jitter_strength: f64,
    pub reorder_packets: bool,
}

impl Default for QoS {
    fn default() -> Self {
        DEFAULT_QOS
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SyntheticFlow {
    pub label: String,
    pub flow: Flow,
    pub link: SyntheticLink,
}

impl Default for SyntheticFlow {
    fn default() -> Self {
        DEFAULT_SYNTHETIC_FLOW
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Flow {
    pub ip: IpPattern,
    pub protocol: Protocol,
    #[serde(flatten)]
    pub port_range: PortRange,
}

impl Default for Flow {
    fn default() -> Self {
        DEFAULT_FLOW
    }
}

#[derive(Serialize_repr, Deserialize_repr, Clone)]
#[repr(u8)]
pub enum Protocol {
    Icmp = 1,
    Tcp = 6,
    Udp = 17,
    All = 0,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(from = "PortRangeSerde", into = "PortRangeSerde")]
pub struct PortRange(RangeInclusive<u16>);

impl PortRange {
    pub fn new(port_min: u16, port_max: u16) -> anyhow::Result<Self> {
        if port_min > port_max {
            bail!("port_min is larger than port_max");
        }
        Ok(PortRange(port_min..=port_max))
    }
}

impl Borrow<RangeInclusive<u16>> for PortRange {
    fn borrow(&self) -> &RangeInclusive<u16> {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct PortRangeSerde {
    port_min: u16,
    port_max: u16,
}

impl From<PortRangeSerde> for PortRange {
    fn from(value: PortRangeSerde) -> Self {
        PortRange(value.port_min..=value.port_max)
    }
}

impl Into<PortRangeSerde> for PortRange {
    fn into(self) -> PortRangeSerde {
        PortRangeSerde {
            port_min: *self.0.start(),
            port_max: *self.0.end(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(from = "IpPatternSerde", into = "IpPatternSerde")]
pub enum IpPattern {
    All,
    Ip(u32),
}

#[derive(Serialize, Deserialize, Clone)]
struct IpPatternSerde(u32);

impl From<IpPatternSerde> for IpPattern {
    fn from(value: IpPatternSerde) -> Self {
        match value.0 {
            0 => IpPattern::All,
            ip => IpPattern::Ip(ip),
        }
    }
}

impl Into<IpPatternSerde> for IpPattern {
    fn into(self) -> IpPatternSerde {
        let ip = match self {
            IpPattern::All => 0,
            IpPattern::Ip(ip) => ip,
        };
        IpPatternSerde(ip)
    }
}

pub struct SyntheticNetworkClient {
    client: Client,
    url: String,
}

impl SyntheticNetworkClient {
    pub fn new(url: String) -> Self {
        SyntheticNetworkClient {
            client: Client::new(),
            url,
        }
    }

    pub async fn commit_config(&mut self, config: &SyntheticNetwork) -> anyhow::Result<()> {
        let result = self.client.post(&self.url).json(config).send().await;
        Ok(result.map(|_| ())?)
    }

    pub async fn load_config(&mut self) -> anyhow::Result<SyntheticNetwork> {
        let result = self.client.get(&self.url).send().await?;
        Ok(result.json::<SyntheticNetwork>().await?)
    }
}
