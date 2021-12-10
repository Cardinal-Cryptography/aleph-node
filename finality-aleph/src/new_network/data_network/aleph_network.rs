use sp_api::BlockT;
use crate::crypto::Signature;
use aleph_bft::SignatureSet;
use crate::data_io::AlephDataFor;
use crate::Hasher;
use crate::data_io::AlephNetworkMessage;
use crate::network::DataNetwork;
use log::error;

pub(crate) type AlephNetworkData<B> =
    aleph_bft::NetworkData<Hasher, AlephDataFor<B>, Signature, SignatureSet<Signature>>;

    /*
impl<B: BlockT> AlephNetworkMessage<B> for AlephNetworkData<B> {
    fn included_blocks(&self) -> Vec<AlephDataFor<B>> {
        self.included_data()
    }
}*/

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
        let recipient = recipient.into();
        if self.inner.send(data, recipient).is_err() {
            error!(target: "afa", "error sending a message to {:?}", recipient);
        }
    }

    async fn next_event(&mut self) -> Option<AlephNetworkData<B>> {
        self.inner.next().await
    }
}