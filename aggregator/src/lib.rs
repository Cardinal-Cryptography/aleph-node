use std::{
    fmt::{Debug, Display},
    hash::Hash as StdHash,
};

use parity_scale_codec::{Codec, Encode};
use aleph_bft_rmc::Message as RmcMessage;
use aleph_bft_types::Recipient;

mod aggregator;

pub use crate::{
    aggregator::{BlockSignatureAggregator, IO},
};

pub type RmcNetworkData<H, S, SS> = RmcMessage<H, S, SS>;

/// A convenience trait for gathering all of the desired hash characteristics.
pub trait Hash: AsRef<[u8]> + StdHash + Eq + Clone + Codec + Debug + Display + Send + Sync {}
impl<T: AsRef<[u8]> + StdHash + Eq + Clone + Codec + Debug + Display + Send + Sync> Hash for T {}

#[derive(Debug)]
pub enum NetworkError {
    SendFail,
}

#[async_trait::async_trait]
pub trait ProtocolSink<D>: Send + Sync {
    async fn next(&mut self) -> Option<D>;
    fn send(&self, data: D, recipient: Recipient) -> Result<(), NetworkError>;
}
