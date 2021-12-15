use crate::{
    aggregator::SignableHash,
    crypto::Signature,
    new_network::data_network::split::DataNetwork,
    Error,
};
use aleph_bft::{SignatureSet, Recipient};
use sp_api::BlockT;

pub(crate) type RmcNetworkData<B> =
    aleph_bft::rmc::Message<SignableHash<<B as BlockT>::Hash>, Signature, SignatureSet<Signature>>;

pub(crate) struct RmcNetwork<B: BlockT> {
    inner: DataNetwork<RmcNetworkData<B>>,
}

impl<B: BlockT> RmcNetwork<B> {
    pub(crate) fn new(inner: DataNetwork<RmcNetworkData<B>>) -> Self {
        RmcNetwork { inner }
    }

    pub(crate) fn send(
        &self,
        data: RmcNetworkData<B>,
        recipient: Recipient,
    ) -> Result<(), Error> {
        self.inner.send(data, recipient)
    }

    pub(crate) async fn next(&mut self) -> Option<RmcNetworkData<B>> {
        self.inner.next().await
    }
}
