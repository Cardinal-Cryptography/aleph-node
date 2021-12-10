use sp_api::BlockT;
use crate::network::DataNetwork;
use crate::NodeIndex;
use crate::Error;
use crate::new_network::data_network::Recipient;
use crate::crypto::Signature;
use crate::aggregator::SignableHash;
use aleph_bft::SignatureSet;

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
        recipient: Recipient<NodeIndex>,
    ) -> Result<(), Error> {
        self.inner.send(data, recipient)
    }

    pub(crate) async fn next(&mut self) -> Option<RmcNetworkData<B>> {
        self.inner.next().await
    }
}