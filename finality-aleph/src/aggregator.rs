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
    Noop, // nothing needs to be done
}

/// A wrapper around an `rmc::Multicast` returning the signed hashes in the order of the [`Multicast::start_multicast`] calls.
pub(crate) struct BlockSignatureAggregator<
    'a,
    H: crate::network::Hash + Copy,
    D: Clone + Codec + Debug + Send + Sync + 'static,
    N: DataNetwork<D>,
    MK: MultiKeychain,
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
        self.rmc.start_multicast(SignableHash { hash }).await;
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
