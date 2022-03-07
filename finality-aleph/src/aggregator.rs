use crate::{
    metrics::Checkpoint,
    network::{DataNetwork, Multicast, Multisigned},
    Metrics,
};
use aleph_bft::{MultiKeychain, Recipient, Signable};
use codec::{Codec, Decode, Encode};
use futures::{channel::mpsc, StreamExt};
use log::{debug, trace, warn};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};

/// A wrapper allowing block hashes to be signed.
#[derive(PartialEq, Eq, Hash, Clone, Debug, Default, Encode, Decode)]
pub struct SignableHash<H: Codec + Send + Sync> {
    pub hash: H,
}

impl<H: AsRef<[u8]> + Hash + Clone + Codec + Send + Sync> Signable for SignableHash<H> {
    type Hash = H;
    fn hash(&self) -> Self::Hash {
        self.hash.clone()
    }
}

/// A type encapsulating three different results of the `process_network_messages` method
pub enum NetworkResult {
    NetworkChannelClosed,
    SignatureInserted,
    Noop,
}

/// A wrapper around an RMC returning the signed hashes in the order of the [`ReliableMulticast::start_rmc`] calls.
pub(crate) struct BlockSignatureAggregator<
    'a,
    H: Copy + Codec + Debug + Eq + Hash + Send + Sync + AsRef<[u8]>, // the hash type
    D: Clone + Codec + Debug + Send + Sync + 'static, // type of data passed through the data network below
    N: DataNetwork<D>,
    MK: MultiKeychain, // multi-keychain
    RMC: Multicast<H>,
> {
    messages_for_rmc: mpsc::UnboundedSender<D>,
    messages_from_rmc: mpsc::UnboundedReceiver<D>,
    signatures: HashMap<H, MK::PartialMultisignature>,
    hash_queue: VecDeque<H>,
    network: N,
    rmc: RMC,
    last_hash_placed: bool,
    started_hashes: HashSet<H>,
    metrics: Option<Metrics<H>>,
    marker: PhantomData<&'a H>,
}

impl<
        'a,
        H: Copy + Codec + Debug + Eq + Hash + Send + Sync + AsRef<[u8]>,
        D: Clone + Codec + Debug + Send + Sync,
        N: DataNetwork<D>,
        MK: MultiKeychain,
        RMC: Multicast<H>,
    > BlockSignatureAggregator<'a, H, D, N, MK, RMC>
where
    RMC::Signed: Multisigned<'a, H, MK>,
{
    pub(crate) fn new(
        network: N,
        rmc: RMC,
        messages_for_rmc: mpsc::UnboundedSender<D>,
        messages_from_rmc: mpsc::UnboundedReceiver<D>,
        metrics: Option<Metrics<H>>,
    ) -> Self {
        // let (messages_for_rmc, messages_from_network) = mpsc::unbounded();
        // let (messages_for_network, messages_from_rmc) = mpsc::unbounded();
        // let scheduler = DoublingDelayScheduler::new(Duration::from_millis(500));
        // let rmc = ReliableMulticast::new(
        //     messages_from_network,
        //     messages_for_network,
        //     keychain,
        //     keychain.node_count(),
        //     scheduler,
        // );
        BlockSignatureAggregator {
            messages_for_rmc,
            messages_from_rmc,
            signatures: HashMap::new(),
            hash_queue: VecDeque::new(),
            network,
            rmc,
            last_hash_placed: false,
            started_hashes: HashSet::new(),
            metrics,
            marker: PhantomData,
        }
    }

    pub(crate) async fn start_aggregation(&mut self, hash: H) {
        debug!(target: "afa", "Started aggregation for block hash {:?}", hash);
        if !self.started_hashes.insert(hash) {
            debug!(target: "afa", "Aggregation already started for block hash {:?}, exiting.", hash);
            return;
        }
        if let Some(metrics) = &self.metrics {
            metrics.report_block(hash, std::time::Instant::now(), Checkpoint::Aggregating);
        }
        self.hash_queue.push_back(hash);
        self.rmc.start_rmc(SignableHash { hash }).await;
    }

    pub(crate) fn notify_last_hash(&mut self) {
        self.last_hash_placed = true;
    }

    pub(crate) async fn process_network_messages(&mut self) -> NetworkResult {
        tokio::select! {
            multisigned_hash = self.rmc.next_multisigned_hash() => {
                let hash = multisigned_hash.as_signable().hash;
                let unchecked = multisigned_hash.into_unchecked().signature();
                debug!(target: "afa", "New multisigned_hash {:?}.", unchecked);
                self.signatures.insert(hash, unchecked);
                return NetworkResult::SignatureInserted;
            }
            message_from_rmc = self.messages_from_rmc.next() => {
                trace!(target: "afa", "Our rmc message {:?}.", message_from_rmc);
                if let Some(message_from_rmc) = message_from_rmc {
                    self.network.send(message_from_rmc, Recipient::Everyone).expect("sending message from rmc failed")
                } else {
                    warn!(target: "afa", "the channel of messages from rmc closed");
                }
            }
            message_from_network = self.network.next() => {
                if let Some(message_from_network) = message_from_network {
                    trace!(target: "afa", "Received message for rmc: {:?}", message_from_network);
                    self.messages_for_rmc.unbounded_send(message_from_network).expect("sending message to rmc failed");
                } else {
                    warn!(target: "afa", "the network channel closed");
                    // In case the network is down we can terminate (?).
                    return NetworkResult::NetworkChannelClosed;
                }
            }
        }
        NetworkResult::Noop
    }

    pub(crate) async fn next_multisigned_hash(&mut self) -> Option<(H, MK::PartialMultisignature)> {
        loop {
            trace!(target: "afa", "Entering next_multisigned_hash loop.");
            match self.hash_queue.front() {
                Some(hash) => {
                    if let Some(multisignature) = self.signatures.remove(hash) {
                        let hash = self
                            .hash_queue
                            .pop_front()
                            .expect("VecDeque::front() returned Some(_), qed.");
                        return Some((hash, multisignature));
                    }
                }
                None => {
                    if self.last_hash_placed {
                        debug!(target: "afa", "Terminating next_multisigned_hash because the last hash has been signed.");
                        return None;
                    }
                }
            }
            loop {
                let res = self.process_network_messages().await;
                match res {
                    NetworkResult::NetworkChannelClosed => {
                        return None;
                    }
                    NetworkResult::SignatureInserted => {
                        break;
                    }
                    NetworkResult::Noop => {
                        continue;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::{AuthorityPen, AuthorityVerifier, KeyBox};
    use crate::network::RmcNetworkData;
    use crate::network::SimpleNetwork;
    use aleph_bft::{NodeIndex, UncheckedSigned};
    use aleph_primitives::{AuthorityId, KEY_TYPE};
    use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
    use sp_keystore::{testing::KeyStore, CryptoStore};
    use sp_runtime::traits::Block;
    use std::sync::Arc;
    use substrate_test_runtime_client::runtime::Block as TBlock;

    use super::*;

    pub type TestHash = <TBlock as Block>::Hash;

    pub struct TestMultisigned {
        hash: SignableHash<TestHash>,
    }

    impl<'a> Multisigned<'a, TestHash, KeyBox> for TestMultisigned {
        fn as_signable(&self) -> &SignableHash<TestHash> {
            &self.hash
        }

        fn into_unchecked(
            self,
        ) -> UncheckedSigned<SignableHash<TestHash>, <KeyBox as MultiKeychain>::PartialMultisignature>
        {
            todo!()
        }
    }

    struct TestMulticast {
        hash: SignableHash<TestHash>,
    }

    #[async_trait::async_trait]
    impl Multicast<TestHash> for TestMulticast {
        type Signed = TestMultisigned;

        async fn start_rmc(&mut self, hash: SignableHash<TestHash>) {}

        fn get_multisigned(&self, hash: &SignableHash<TestHash>) -> Option<Self::Signed> {
            Some(TestMultisigned { hash: hash.clone() })
        }

        async fn next_multisigned_hash(&mut self) -> Self::Signed {
            TestMultisigned {
                hash: self.hash.clone(),
            }
        }
    }

    #[tokio::test]
    async fn should_test_something() {
        let (sender_tx, _sender_rx) = mpsc::unbounded::<RmcNetworkData<TBlock>>();
        let (network_tx, network_rx) = mpsc::unbounded::<RmcNetworkData<TBlock>>();
        let test_network = SimpleNetwork::new(network_rx, sender_tx);

        let names = vec![String::from("//Alice"), String::from("//Bob")];
        let key_store = Arc::new(KeyStore::new());

        let mut authority_ids = Vec::with_capacity(names.len());
        for name in &names {
            let pk = key_store
                .ed25519_generate_new(KEY_TYPE, Some(name))
                .await
                .expect("Failed to generate the key");
            authority_ids.push(AuthorityId::from(pk));
        }

        let mut pens = Vec::with_capacity(names.len());
        for authority_id in authority_ids.clone() {
            pens.push(
                AuthorityPen::new(authority_id, key_store.clone())
                    .await
                    .expect("The keys should sign successfully"),
            );
        }

        let verifier = AuthorityVerifier::new(authority_ids.clone());
        let key_box = KeyBox::new(NodeIndex(0), verifier, pens[0].clone());
        // let key_box = TestKeyBox {};
        let rmc = TestMulticast {
            hash: Default::default(),
        };
        let (messages_for_rmc, messages_from_network) = mpsc::unbounded();
        let (messages_for_network, messages_from_rmc) = mpsc::unbounded();
        let aggregator =
            BlockSignatureAggregator::<
                TestHash,
                RmcNetworkData<TBlock>,
                SimpleNetwork<
                    RmcNetworkData<TBlock>,
                    UnboundedReceiver<RmcNetworkData<TBlock>>,
                    UnboundedSender<RmcNetworkData<TBlock>>,
                >,
                KeyBox,
                TestMulticast,
            >::new(test_network, rmc, messages_for_rmc, messages_from_rmc, None);

        // let sig = pens[0].sign(b"test").await;
        // let sig2 = Signed::sign(key_box, &key_box).await;
        // let msg = Message::SignedHash(sig2.into());
        // network_tx.unbounded_send();

        assert_eq!(1, 1);
    }
}
