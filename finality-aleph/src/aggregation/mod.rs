use std::{marker::PhantomData, time::Instant};

use aleph_aggregator::NetworkError;
use aleph_bft::{Recipient, SignatureSet};
use sp_runtime::traits::Block;

use crate::{
    crypto::Signature,
    metrics::{Checkpoint, Key},
    network::{Data, DataNetwork, SendError},
    Metrics,
};

pub type RmcNetworkData<B> =
    aleph_aggregator::RmcNetworkData<<B as Block>::Hash, Signature, SignatureSet<Signature>>;

pub struct NetworkWrapper<D: Data, N: DataNetwork<D>>(N, PhantomData<D>);
pub struct MetricsWrapper<H: Key, M: Metrics<H>>(M, PhantomData<H>);

impl<D: Data, N: DataNetwork<D>> NetworkWrapper<D, N> {
    pub fn new(network: N) -> Self {
        Self(network, PhantomData)
    }
}

impl<H: Key, M: Metrics<H>> MetricsWrapper<H, M> {
    pub fn new(metrics: M) -> Self {
        Self(metrics, PhantomData)
    }
}

impl<H: Key, M: Metrics<H>> aleph_aggregator::Metrics<H> for MetricsWrapper<H, M> {
    fn report_aggregation_complete(&mut self, h: H) {
        self.0
            .report_block(h, Instant::now(), Checkpoint::Aggregating);
    }
}

#[async_trait::async_trait]
impl<T, D> aleph_aggregator::ProtocolSink<D> for NetworkWrapper<D, T>
where
    T: DataNetwork<D>,
    D: Data,
{
    async fn next(&mut self) -> Option<D> {
        self.0.next().await
    }

    fn send(&self, data: D, recipient: Recipient) -> Result<(), NetworkError> {
        self.0.send(data, recipient).map_err(|e| match e {
            SendError::SendFailed => NetworkError::SendFail,
        })
    }
}
