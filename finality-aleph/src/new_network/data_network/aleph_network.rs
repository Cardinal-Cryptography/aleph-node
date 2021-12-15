use crate::{
    crypto::Signature,
    data_io::{AlephDataFor, AlephNetworkMessage},
    new_network::data_network::split::DataNetwork,
    Hasher,
};
use aleph_bft::SignatureSet;
use log::warn;
use sp_api::BlockT;

pub(crate) type AlephNetworkData<B> =
    aleph_bft::NetworkData<Hasher, AlephDataFor<B>, Signature, SignatureSet<Signature>>;

impl<B: BlockT> AlephNetworkMessage<B> for AlephNetworkData<B> {
    fn included_blocks(&self) -> Vec<AlephDataFor<B>> {
        self.included_data()
    }
}

pub(crate) struct AlephNetwork<B: BlockT> {
    inner: DataNetwork<AlephNetworkData<B>>,
}

impl<B: BlockT> AlephNetwork<B> {
    pub(crate) fn new(inner: DataNetwork<AlephNetworkData<B>>) -> Self {
        AlephNetwork { inner }
    }
}

#[async_trait::async_trait]
impl<B: BlockT> aleph_bft::Network<Hasher, AlephDataFor<B>, Signature, SignatureSet<Signature>>
    for AlephNetwork<B>
{
    fn send(&self, data: AlephNetworkData<B>, recipient: aleph_bft::Recipient) {
        if self.inner.send(data, recipient.clone()).is_err() {
            warn!(target: "afa", "error sending a message to {:?}", recipient);
        }
    }

    async fn next_event(&mut self) -> Option<AlephNetworkData<B>> {
        self.inner.next().await
    }
}
