use crate::{network::AlephNetworkData, Error, Metrics};
use aleph_bft::OrderedBatch;
use futures::channel::{mpsc, oneshot};
use parking_lot::Mutex;
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};
use std::{sync::Arc, time::Duration};

const REFRESH_INTERVAL: u64 = 100;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_timer::Delay;
use log::{debug};
use sc_client_api::{backend::Backend, BlockchainEvents};
use sp_runtime::generic::BlockId;
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};
use tokio::stream::StreamExt;

type MessageId = u64;

pub(crate) struct DataStore<B, C, BE>
where
    B: Block,
    C: crate::ClientForAleph<B, BE> + BlockchainEvents<B> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    BE: Backend<B> + 'static,
{
    // TODO: keep at most certain amount of blocks
    next_message_id: MessageId,
    messages_for_member: UnboundedSender<AlephNetworkData<B>>,
    // TODO: Change name. This name is stupid
    messages_for_store: UnboundedReceiver<AlephNetworkData<B>>,
    // TODO: Change to HashSet when HashSet is empty remove block
    dependent_messages: HashMap<B::Hash, Vec<MessageId>>,
    // TODO: Change to LRU cache
    available_blocks: HashSet<B::Hash>,
    message_requirements: HashMap<MessageId, usize>,
    pending_messages: HashMap<MessageId, AlephNetworkData<B>>,
    client: Arc<C>,
    _phantom: PhantomData<BE>,
}

impl<B, C, BE> DataStore<B, C, BE>
where
    B: Block,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static + BlockchainEvents<B>,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    BE: Backend<B> + 'static,
{
    pub(crate) fn new(
        client: Arc<C>,
        messages_for_member: UnboundedSender<AlephNetworkData<B>>,
        messages_for_store: UnboundedReceiver<AlephNetworkData<B>>,
    ) -> Self {
        DataStore {
            next_message_id: 0,
            client,
            message_requirements: HashMap::new(),
            dependent_messages: HashMap::new(),
            pending_messages: HashMap::new(),
            available_blocks: HashSet::new(),
            messages_for_member,
            messages_for_store,
            _phantom: PhantomData,
        }
    }

    pub(crate) async fn run(&mut self, mut exit: oneshot::Receiver<()>) {
        let mut timeout = Delay::new(Duration::from_millis(5000));
        let mut import_stream = self.client.import_notification_stream();
        loop {
            tokio::select! {
                Some(message) = &mut self.messages_for_store.next() => {
                    debug!(target: "afa", "Got a message at DataStore {:?}", message);
                    self.add_message(message);
                }
                Some(block) = &mut import_stream.next() => {
                    debug!(target: "afa", "Block import notification {:?}", block);
                    self.add_block(block.hash);
                }
                _ = &mut timeout => {
                    debug!(target: "afa", "Timout Timeout Timeout");
                    let keys : Vec<_> = self.dependent_messages.keys().cloned().collect();
                    for block_hash in keys {
                        if self.client.header(BlockId::Hash(block_hash)).is_ok() {
                            self.add_block(block_hash);
                        }
                    }
                    timeout = Delay::new(Duration::from_millis(5000));
                }
                _ = &mut exit => {
                    break;
                }
            }
        }
    }

    fn add_pending_message(&mut self, message: AlephNetworkData<B>, requirements: Vec<B::Hash>) {
        let message_id = self.next_message_id;
        // Whatever test you are running should end before this becomes a problem.
        self.next_message_id += 1;
        for block_num in requirements.iter() {
            self.dependent_messages
                .entry(*block_num)
                .or_insert_with(Vec::new)
                .push(message_id);
        }
        self.message_requirements
            .insert(message_id, requirements.len());
        self.pending_messages.insert(message_id, message);
    }

    pub(crate) fn add_message(&mut self, message: AlephNetworkData<B>) {
        let requirements: Vec<_> = message
            .included_data()
            .into_iter()
            .filter(|b| {
                if self.available_blocks.contains(b) {
                    return false;
                }
                if self.client.header(BlockId::Hash(*b)).is_ok() {
                    self.available_blocks.insert(*b);
                    return false;
                }
                true
            })
            .collect();
        if requirements.is_empty() {
            debug!(target: "afa", "Sent message from DataStore {:?}", message);
            self.messages_for_member
                .unbounded_send(message)
                .expect("member accept messages");
        } else {
            self.add_pending_message(message, requirements);
        }
    }

    fn push_messages(&mut self, block_hash: B::Hash) {
        for message_id in self
            .dependent_messages
            .entry(block_hash)
            .or_insert_with(Vec::new)
            .iter()
        {
            *self
                .message_requirements
                .get_mut(message_id)
                .expect("there are some requirements") -= 1;
            if self.message_requirements[message_id] == 0 {
                let message = self
                    .pending_messages
                    .remove(message_id)
                    .expect("there is a pending message");
                self.messages_for_member
                    .unbounded_send(message)
                    .expect("member accept messages");
                self.message_requirements.remove(message_id);
            }
        }
        self.dependent_messages.remove(&block_hash);
    }

    pub(crate) fn add_block(&mut self, block_hash: B::Hash) {
        debug!(target: "data-store", "Added block {:?}.", block_hash);
        self.available_blocks.insert(block_hash);
        self.push_messages(block_hash);
    }
}

#[derive(Clone)]
pub(crate) struct DataIO<B: Block> {
    pub(crate) best_chain: Arc<Mutex<B::Hash>>,
    pub(crate) ordered_batch_tx: mpsc::UnboundedSender<OrderedBatch<B::Hash>>,
    pub(crate) metrics: Option<Metrics<B::Header>>,
}

pub(crate) async fn refresh_best_chain<B: Block, SC: SelectChain<B>>(
    select_chain: SC,
    best_chain: Arc<Mutex<B::Hash>>,
    mut exit: oneshot::Receiver<()>,
) {
    loop {
        let delay = futures_timer::Delay::new(Duration::from_millis(REFRESH_INTERVAL));
        tokio::select! {
            _ = delay => {
                let new_best_chain = select_chain
                    .best_chain()
                    .await
                    .expect("No best chain")
                    .hash();
                *best_chain.lock() = new_best_chain;
            }
            _ = &mut exit => {
                debug!(target: "afa", "Task for refreshing best chain received exit signal. Terminating.");
                return;
            }
        }
    }
}

impl<B: Block> aleph_bft::DataIO<B::Hash> for DataIO<B> {
    type Error = Error;

    fn get_data(&self) -> B::Hash {
        let hash = *self.best_chain.lock();

        if let Some(m) = &self.metrics {
            m.report_block(hash, std::time::Instant::now(), "get_data");
        }
        debug!(target: "afa", "Outputting {:?} in get_data", hash);
        hash
    }

    fn send_ordered_batch(&mut self, batch: OrderedBatch<B::Hash>) -> Result<(), Self::Error> {
        // TODO: add better conversion
        self.ordered_batch_tx
            .unbounded_send(batch)
            .map_err(|_| Error::SendData)
    }
}
