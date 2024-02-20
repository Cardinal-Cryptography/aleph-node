use std::{
    collections::{hash_map::Entry::Occupied, BTreeMap, HashMap, HashSet},
    default::Default,
    fmt::Display,
    hash::Hash,
    num::NonZeroUsize,
    sync::Arc,
    time::{self, Duration},
};

use futures::channel::{
    mpsc::{self, UnboundedSender},
    oneshot,
};
use futures_timer::Delay;
use log::{debug, error, info, trace, warn};
use lru::LruCache;

use crate::{
    aleph_primitives::BlockNumber,
    block::{
        BlockchainEvents, ChainStatusNotification, ChainStatusNotifier, Header, HeaderBackend,
        HeaderVerifier, UnverifiedHeader,
    },
    data_io::{
        chain_info::{CachedChainInfoProvider, ChainInfoProvider, SubstrateChainInfoProvider},
        proposal::{AlephProposal, PendingProposalStatus, ProposalStatus},
        status_provider::get_proposal_status,
        AlephNetworkMessage,
    },
    network::data::{
        component::{Network as ComponentNetwork, Receiver, SimpleNetwork},
        Network as DataNetwork,
    },
    party::manager::Runnable,
    sync::RequestBlocks,
    BlockId, SessionBoundaries,
};

const LOG_TARGET: &str = "aleph-data-store";

type MessageId = u64;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ChainEvent {
    Imported(BlockId),
    Finalized(BlockNumber),
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PendingProposalInfo {
    // Which messages are being held because of a missing the data item.
    messages: HashSet<MessageId>,
    // When was the first message containing this data item encountered.
    first_occurrence: time::SystemTime,
    status: ProposalStatus,
}

impl PendingProposalInfo {
    fn new(status: ProposalStatus) -> Self {
        PendingProposalInfo {
            messages: HashSet::new(),
            first_occurrence: time::SystemTime::now(),
            status,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PendingMessageInfo<UH: UnverifiedHeader, M: AlephNetworkMessage<UH>> {
    message: M,
    // Data items that we still wait for
    pending_proposals: HashSet<AlephProposal<UH>>,
}

impl<UH: UnverifiedHeader, M: AlephNetworkMessage<UH>> PendingMessageInfo<UH, M> {
    fn new(message: M) -> Self {
        PendingMessageInfo {
            message,
            pending_proposals: HashSet::new(),
        }
    }
}

pub struct DataStoreConfig {
    pub max_triggers_pending: usize,
    pub max_proposals_pending: usize,
    pub max_messages_pending: usize,
    pub available_proposals_cache_capacity: NonZeroUsize,
    pub periodic_maintenance_interval: Duration,
    // Specifies how much time must pass from receiving a given proposal for the first time, till we
    // perform a request for either a block or a justification required to let this proposal through.
    pub request_block_after: Duration,
}

impl Default for DataStoreConfig {
    fn default() -> DataStoreConfig {
        DataStoreConfig {
            max_triggers_pending: 80_000,
            max_proposals_pending: 80_000,
            max_messages_pending: 40_000,
            available_proposals_cache_capacity: NonZeroUsize::new(8000).unwrap(),
            periodic_maintenance_interval: Duration::from_secs(25),
            request_block_after: Duration::from_secs(20),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    ChainNotificationsTerminated,
    NetworkMessagesTerminated,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ChainNotificationsTerminated => {
                write!(f, "Chain notifications stream was closed.")
            }
            Error::NetworkMessagesTerminated => {
                write!(f, "Stream with network messages was closed.")
            }
        }
    }
}

// DataStore is the data availability proxy for the AlephBFT protocol, meaning that whenever we receive
// a message `m` we must check whether the data `m.included_data()` is available to pass it to AlephBFT.
// Data is represented by the `AlephData` type -- we refer to the docs of this type to learn what
// it represents and how honest nodes form `AlephData` instances.
// An `AlephData` is considered available if it is either `Empty` or it is `HeadProposal(p)` where
// `p` is a proposal satisfying one of the conditions below:
// 1) the top block of `p`s branch is available AND the branch is correct (hashes correspond to existing blocks
//    with correct number and the ancestry is correct) AND the parent of the bottom block in the branch is finalized.
// 2) (Hopeless Fork) There exists a hash h_i on the branch, corresponding to height `num` in the chain, such that
//    some block `b` of number `num` is finalized and `hash(b) != h`. This is simply a situation in which the proposal
//    no matter whether honest or not, cannot possibly be applied, as a conflicting block was already finalized.
// It is possible that both 1) and 2) might be true for some proposals, but that's fine.
//
// The way DataStore works internally, is roughly as follows:
// 1) We keep receiving and caching imported and finalized blocks via appropriate subscriptions from the client. It is
//    worth noting that these subscriptions are not 100% accurate and it might happen that some blocks are not included.
//    That's why we "trust" them in the optimistic case, but also have a fallback mechanism in which we query the client
//    directly for imported and finalized blocks. There are two reasons why don't we use just the client:
//    a) performance -- the queries to the client are slow, because they use the DB and possibly touch the disk
//    b) using the subscriptions allows us to make the store "event-based", i.e., at the very moment a block is imported
//       we can immediately process all proposals pending because of this block. This would be impossible to do
//       by just querying the client.
// 2) Once a message `m` arrives. We extract all the proposals in `m` and we check whether all of them are available.
//    In case any of them is not, the message `m` receives a fresh `MessageId` and the message and all the pending
//    proposals are added to our "pending list". The dependencies between the message and the proposals it waits for
//    are also tracked. At the very first moment when the last pending proposal of the message becomes available, the
//    message is removed from the pending list and is output on "the other side" of DataStore.
// 3) It is crucial for DataStore to use a bounded amount of memory, which is perhaps the hardest challenge when implementing it.
//    There are constants in the `DataStoreConfig` that determine maximum possible amounts of messages and proposals that
//    can be pending at the same time. When any of the limits is exceeded, we keep dropping messages (starting from
//    the oldest) until it is fine again.
// 4) To be able to quickly realize availability of pending proposals we use a mechanism of "bumping" the proposals based
//    on some events. Each proposal has some events registered (either block import or block finalization) that once triggered
//    will "bump" the proposal and it will be checked for availability again.
// 5) Periodically, every `config.periodic_maintenance_interval` time we run "maintenance" which has two purposes:
//    a) To bump long-pending proposals that for some reason are still in the data store -- maybe because some blocks
//       were missed by the block import subscription.
//    b) To explicitly request blocks that are the cause of some proposals pending for a long time.

/// This component is used for filtering available data for Aleph Network.
/// It needs to be started by calling the run method.
pub struct DataStore<H, HB, BEV, RB, Message, R, V>
where
    H: Header,
    HB: HeaderBackend<H> + 'static,
    BEV: BlockchainEvents<H>,
    RB: RequestBlocks<H::Unverified>,
    Message: AlephNetworkMessage<H::Unverified>
        + std::fmt::Debug
        + Send
        + Sync
        + Clone
        + parity_scale_codec::Codec
        + 'static,
    R: Receiver<Message>,
    V: HeaderVerifier<H>,
{
    next_free_id: MessageId,
    pending_proposals: HashMap<AlephProposal<H::Unverified>, PendingProposalInfo>,
    event_triggers: HashMap<ChainEvent, HashSet<AlephProposal<H::Unverified>>>,
    // We use BtreeMap instead of HashMap to be able to fetch the Message with lowest MessageId
    // when pruning messages.
    pending_messages: BTreeMap<MessageId, PendingMessageInfo<H::Unverified, Message>>,
    chain_info_provider: CachedChainInfoProvider<SubstrateChainInfoProvider<H, HB>>,
    verifier: V,
    available_proposals_cache: LruCache<AlephProposal<H::Unverified>, ProposalStatus>,
    num_triggers_registered_since_last_pruning: usize,
    highest_finalized_num: BlockNumber,
    session_boundaries: SessionBoundaries,
    blockchain_events: Arc<BEV>,
    block_requester: RB,
    config: DataStoreConfig,
    messages_from_network: R,
    messages_for_aleph: UnboundedSender<Message>,
}

impl<H, HB, BEV, RB, Message, R, V> DataStore<H, HB, BEV, RB, Message, R, V>
where
    H: Header,
    HB: HeaderBackend<H>,
    BEV: BlockchainEvents<H>,
    RB: RequestBlocks<H::Unverified>,
    Message: AlephNetworkMessage<H::Unverified>
        + std::fmt::Debug
        + Send
        + Sync
        + Clone
        + parity_scale_codec::Codec
        + 'static,
    R: Receiver<Message>,
    V: HeaderVerifier<H>,
{
    /// Returns a struct to be run and a network that outputs messages filtered as appropriate
    pub fn new<N: ComponentNetwork<Message, R = R>>(
        session_boundaries: SessionBoundaries,
        header_backend: HB,
        blockchain_events: Arc<BEV>,
        verifier: V,
        block_requester: RB,
        config: DataStoreConfig,
        component_network: N,
    ) -> (Self, impl DataNetwork<Message>) {
        let (messages_for_aleph, messages_from_data_store) = mpsc::unbounded();
        let (messages_to_network, messages_from_network) = component_network.into();
        let highest_finalized_num = header_backend.top_finalized_id().number();
        let chain_info_provider = CachedChainInfoProvider::new(
            SubstrateChainInfoProvider::new(header_backend),
            Default::default(),
        );

        (
            DataStore {
                next_free_id: 0,
                pending_proposals: HashMap::new(),
                event_triggers: HashMap::new(),
                pending_messages: BTreeMap::new(),
                chain_info_provider,
                verifier,
                available_proposals_cache: LruCache::new(config.available_proposals_cache_capacity),
                num_triggers_registered_since_last_pruning: 0,
                highest_finalized_num,
                session_boundaries,
                blockchain_events,
                block_requester,
                config,
                messages_from_network,
                messages_for_aleph,
            },
            SimpleNetwork::new(messages_from_data_store, messages_to_network),
        )
    }

    pub async fn run(&mut self, mut exit: oneshot::Receiver<()>) -> Result<(), Error> {
        let mut maintenance_clock = Delay::new(self.config.periodic_maintenance_interval);
        let mut chain_status_notifier = self.blockchain_events.chain_status_notifier();
        loop {
            self.prune_pending_messages();
            self.prune_triggers();
            tokio::select! {
                maybe_message = self.messages_from_network.next() => {
                    let message = maybe_message.ok_or(Error::NetworkMessagesTerminated)?;
                    trace!(target: LOG_TARGET, "Received message at Data Store {:?}", message);
                    self.on_message_received(message);
                }
                maybe_notification = chain_status_notifier.next() => {
                    let notification =
                        maybe_notification.map_err(|_| Error::ChainNotificationsTerminated)?;
                    match notification {
                        ChainStatusNotification::BlockImported(header) => {
                            trace!(target: LOG_TARGET, "Block import notification at Data Store for block with header {:?}", header);
                            self.on_block_imported(header.id());
                        }
                        ChainStatusNotification::BlockFinalized(header) => {
                            trace!(target: LOG_TARGET, "Finalized block import notification at Data Store for block with header {:?}", header);
                            self.on_block_finalized(header.id());
                        }
                    }
                }
                _ = &mut maintenance_clock => {
                    self.run_maintenance();
                    maintenance_clock = Delay::new(self.config.periodic_maintenance_interval);
                }
                _ = &mut exit => {
                    debug!(target: LOG_TARGET, "Data Store task received exit signal. Terminating.");
                    break;
                }
            }
        }
        debug!(target: LOG_TARGET, "Data store finished");
        Ok(())
    }

    // Updates our highest known and highest finalized block info directly from the client.
    fn update_highest_finalized(&mut self) {
        let highest_finalized = self.chain_info_provider.get_highest_finalized();
        self.on_block_imported(highest_finalized.clone());
        self.on_block_finalized(highest_finalized);
    }

    fn run_maintenance(&mut self) {
        self.update_highest_finalized();

        let proposals_with_timestamps: Vec<_> = self
            .pending_proposals
            .iter()
            .map(|(proposal, info)| (proposal.clone(), info.first_occurrence))
            .collect();
        match proposals_with_timestamps.len() {
            0 => {
                trace!(
                    target: LOG_TARGET,
                    "No pending proposals in data store during maintenance."
                );
            }
            1..=5 => {
                info!(
                    target: LOG_TARGET,
                    "Data Store maintenance. Awaiting {:?} proposals: {:?}",
                    proposals_with_timestamps.len(),
                    proposals_with_timestamps
                );
            }
            _ => {
                info!(target: LOG_TARGET, "Data Store maintenance. Awaiting {:?} proposals: (showing 5 initial only) {:?}",proposals_with_timestamps.len(), &proposals_with_timestamps[..5]);
            }
        }

        let now = time::SystemTime::now();
        for (proposal, first_occurrence) in proposals_with_timestamps {
            if self.bump_proposal(&proposal) {
                continue;
            }
            // `bump_proposal` returns false if the bump didn't make the proposal available, meaning that it is still pending
            let time_waiting = match now.duration_since(first_occurrence) {
                Ok(tw) if tw >= self.config.request_block_after => tw,
                _ => continue,
            };

            let header = proposal.top_block_header();
            let block_id = proposal.top_block();
            if !self.chain_info_provider.is_block_imported(&block_id) {
                debug!(
                    target: LOG_TARGET,
                    "Requesting a block {:?} after it has been missing for {:?} secs.",
                    block_id,
                    time_waiting.as_secs()
                );
                if let Err(e) = self.block_requester.request_block(header) {
                    warn!(
                        target: LOG_TARGET,
                        "Error requesting block {:?}, {}.", block_id, e
                    );
                }
                continue;
            }
            // The top block (thus the whole branch, in the honest case) has been imported. What's holding us
            // must be that the parent of the base is not finalized. This might be either because of a malicious
            // proposal (with not finalized "base") or because we are not up-to-date with finalization.
            let bottom_block = proposal.bottom_block();
            let parent_hash = match self.chain_info_provider.get_parent_hash(&bottom_block) {
                Ok(ph) => ph,
                _ => {
                    warn!(
                        target: LOG_TARGET,
                        "Expected the block below the proposal {:?} to be imported", proposal
                    );
                    continue;
                }
            };
            let parent_num = bottom_block.number() - 1;
            if let Ok(finalized_block) = self.chain_info_provider.get_finalized_at(parent_num) {
                if parent_hash != finalized_block.hash() {
                    warn!(
                        target: LOG_TARGET,
                        "The proposal {:?} is pending because the parent: \
                        {:?}, does not agree with the block finalized at this height: {:?}.",
                        proposal,
                        parent_hash,
                        finalized_block
                    );
                } else {
                    warn!(
                        target: LOG_TARGET,
                        "The proposal {:?} is pending even though blocks \
                            have been imported and parent was finalized.",
                        proposal
                    );
                }
            } else {
                debug!(
                    target: LOG_TARGET,
                    "Justification for block {:?} {:?} \
                        still not present after {:?} secs.",
                    parent_num,
                    parent_hash,
                    time_waiting.as_secs()
                );
            }
        }
    }

    fn register_block_import_trigger(
        &mut self,
        proposal: &AlephProposal<H::Unverified>,
        block: &BlockId,
    ) {
        self.num_triggers_registered_since_last_pruning += 1;
        self.event_triggers
            .entry(ChainEvent::Imported(block.clone()))
            .or_default()
            .insert(proposal.clone());
    }

    fn register_finality_trigger(
        &mut self,
        proposal: &AlephProposal<H::Unverified>,
        number: BlockNumber,
    ) {
        self.num_triggers_registered_since_last_pruning += 1;
        if number > self.highest_finalized_num {
            self.event_triggers
                .entry(ChainEvent::Finalized(number))
                .or_default()
                .insert(proposal.clone());
        }
    }

    fn register_next_finality_trigger(&mut self, proposal: &AlephProposal<H::Unverified>) {
        if self.highest_finalized_num < proposal.number_below_branch() {
            self.register_finality_trigger(proposal, proposal.number_below_branch());
        } else if self.highest_finalized_num < proposal.number_top_block() {
            self.register_finality_trigger(proposal, self.highest_finalized_num + 1);
        }
    }

    fn on_block_finalized(&mut self, block: BlockId) {
        if self.highest_finalized_num < block.number() {
            // We don't assume block.num = self.highest_finalized_num + 1 as the finality import queue does
            // not quite guarantee this.
            let old_num = self.highest_finalized_num;
            let new_num = block.number();
            self.highest_finalized_num = new_num;
            // We activate all finality triggers in [old_num + 1, block.num].
            let mut num = old_num + 1;
            while num <= new_num {
                if let Some(proposals_to_bump) =
                    self.event_triggers.remove(&ChainEvent::Finalized(num))
                {
                    for proposal in proposals_to_bump {
                        self.bump_proposal(&proposal);
                    }
                }
                num += 1;
            }
        }
    }

    fn on_block_imported(&mut self, block: BlockId) {
        if let Some(proposals_to_bump) = self.event_triggers.remove(&ChainEvent::Imported(block)) {
            for proposal in proposals_to_bump {
                self.bump_proposal(&proposal);
            }
        }
    }

    fn on_proposal_available(&mut self, proposal: &AlephProposal<H::Unverified>) {
        if let Some(proposal_info) = self.pending_proposals.remove(proposal) {
            for id in proposal_info.messages {
                self.remove_proposal_from_pending_message(proposal, id);
            }
        }
    }

    // Makes an availability check for `data` and updates its status. Outputs whether the bump resulted in
    // this proposal becoming available.
    fn bump_proposal(&mut self, proposal: &AlephProposal<H::Unverified>) -> bool {
        // Some minor inefficiencies in HashMap access below because of borrow checker.
        let old_status = match self.pending_proposals.get(proposal) {
            None => {
                // It is possible that `proposal` is not pending anymore, as it has become available as a result of
                // an earlier bump.
                return false;
            }
            Some(info) => info.status.clone(),
        };
        let new_status = self.check_proposal_availability(proposal, Some(&old_status));
        self.pending_proposals.get_mut(proposal).unwrap().status = new_status.clone();

        use PendingProposalStatus::*;
        use ProposalStatus::*;
        match new_status {
            Pending(PendingTopBlock) => {
                // We register only a finality trigger, since a block import trigger has been already registered
                // when the proposal was added.
                self.register_next_finality_trigger(proposal);
                false
            }
            Pending(TopBlockImportedButIncorrectBranch) => {
                // We do nothing, this is a result of malicious behaviour. This proposal can still get through,
                // but we wait till maintenance.
                false
            }
            Pending(TopBlockImportedButNotFinalizedAncestor) => {
                // This is possible in honest scenarios -- this node must be behind in finalization.
                self.register_next_finality_trigger(proposal);
                false
            }
            Finalize(_) | Ignore => {
                self.on_proposal_available(proposal);
                true
            }
        }
    }

    // Outputs the current status of the proposal based on the `old_status` (for optimization).
    fn check_proposal_availability(
        &mut self,
        proposal: &AlephProposal<H::Unverified>,
        old_status: Option<&ProposalStatus>,
    ) -> ProposalStatus {
        if let Some(status) = self.available_proposals_cache.get(proposal) {
            return status.clone();
        }
        let status = get_proposal_status(
            &mut self.chain_info_provider,
            &mut self.verifier,
            proposal,
            old_status,
        );
        match status {
            ProposalStatus::Finalize(_) | ProposalStatus::Ignore => {
                // We can cache only if the proposal is available. If it is pending, its
                // status might change and we should not recover it from the cache.
                self.available_proposals_cache
                    .put(proposal.clone(), status.clone());
            }
            _ => {}
        }
        status
    }

    // For a proposal that might be new or not, check if it is available. If it is a new proposal
    // and it is not available:
    // 1) create an entry in pending_proposals
    // 2) link this proposal to the input message by modifying the new added entry in pending proposals
    //    and the provided message_info.
    // 3) register an appropriate event trigger (for block import and/or finality).
    // If the proposal is available, message_info is not modified.
    fn add_message_proposal_dependency(
        &mut self,
        proposal: &AlephProposal<H::Unverified>,
        message_info: &mut PendingMessageInfo<H::Unverified, Message>,
        id: MessageId,
    ) {
        if !self.pending_proposals.contains_key(proposal) {
            use PendingProposalStatus::*;
            use ProposalStatus::*;
            let status = self.check_proposal_availability(proposal, None);
            match &status {
                Pending(PendingTopBlock) => {
                    self.pending_proposals
                        .insert(proposal.clone(), PendingProposalInfo::new(status));
                    self.register_block_import_trigger(proposal, &proposal.top_block());
                    self.register_next_finality_trigger(proposal);
                }
                Pending(TopBlockImportedButIncorrectBranch) => {
                    self.pending_proposals
                        .insert(proposal.clone(), PendingProposalInfo::new(status));
                    // The only way this might ever get through is as a hopeless fork. So the only event that might
                    // change the status of this proposal is a finalization event, hence we register a trigger.
                    self.register_next_finality_trigger(proposal);
                }
                Pending(TopBlockImportedButNotFinalizedAncestor) => {
                    self.pending_proposals
                        .insert(proposal.clone(), PendingProposalInfo::new(status));

                    self.register_next_finality_trigger(proposal);
                }

                Finalize(_) | Ignore => {
                    // Proposal available, no need to register any dependencies
                    return;
                }
            }
        }
        // This line is reached only if the proposal is not available
        let proposal_info = self
            .pending_proposals
            .get_mut(proposal)
            .expect("exists as checked above");
        proposal_info.messages.insert(id);
        message_info.pending_proposals.insert(proposal.clone());
    }

    fn on_message_dependencies_resolved(&self, message: Message) {
        trace!(
            target: LOG_TARGET,
            "Sending message from DataStore {:?}",
            message
        );
        if let Err(e) = self.messages_for_aleph.unbounded_send(message) {
            error!(
                target: LOG_TARGET,
                "Unable to send a ready message from DataStore {}", e
            );
        }
    }

    fn assign_fresh_message_id(&mut self) -> MessageId {
        self.next_free_id += 1;
        self.next_free_id - 1
    }

    // This is called upon a proposal being available -- we remove it from the set of
    // proposals a message waits for.
    fn remove_proposal_from_pending_message(
        &mut self,
        proposal: &AlephProposal<H::Unverified>,
        id: MessageId,
    ) {
        let mut message_info = match self.pending_messages.remove(&id) {
            Some(message_info) => message_info,
            None => {
                warn!(
                    target: LOG_TARGET,
                    "Message {:?} not found when resolving a proposal dependency {:?}.",
                    id,
                    proposal
                );
                return;
            }
        };
        message_info.pending_proposals.remove(proposal);
        if message_info.pending_proposals.is_empty() {
            self.on_message_dependencies_resolved(message_info.message);
        } else {
            // We reinsert the message because it still has pending proposals.
            self.pending_messages.insert(id, message_info);
        }
    }

    fn remove_message_id_from_pending_proposal(
        &mut self,
        proposal: &AlephProposal<H::Unverified>,
        id: MessageId,
    ) {
        if let Occupied(mut proposal_entry) = self.pending_proposals.entry(proposal.clone()) {
            let proposal_info = proposal_entry.get_mut();
            proposal_info.messages.remove(&id);
            if proposal_info.messages.is_empty() {
                proposal_entry.remove();
            }
        } else {
            warn!(
                target: LOG_TARGET,
                "Proposal {:?} with id {:?} referenced in message does not exist", proposal, id
            );
        }
    }

    fn prune_single_message(&mut self) -> bool {
        let maybe_id = self.pending_messages.keys().next().cloned();
        if let Some(id) = maybe_id {
            if let Some(message_info) = self.pending_messages.remove(&id) {
                for proposal in message_info.pending_proposals {
                    self.remove_message_id_from_pending_proposal(&proposal, id);
                }
                true
            } else {
                warn!(
                    "Trying to prune a message whose id is not in pending messages {:?}",
                    id
                );
                false
            }
        } else {
            warn!(
                target: LOG_TARGET,
                "Tried to prune a message but there are none pending."
            );
            false
        }
    }

    // Checks if we have exceeded the maximum number of pending messages or proposals.
    // If so, we prune messages until the limits are satisfied again.
    fn prune_pending_messages(&mut self) {
        while self.pending_messages.len() > self.config.max_messages_pending
            || self.pending_proposals.len() > self.config.max_proposals_pending
        {
            if !self.prune_single_message() {
                warn!(
                    target: LOG_TARGET,
                    "Message pruning in DataStore failed. Moving on."
                );
                break;
            }
        }
    }

    fn prune_triggers(&mut self) {
        if self.num_triggers_registered_since_last_pruning > self.config.max_triggers_pending {
            // Prune all the data that is not pending anymore and all the events that
            // have an empty list if triggers.
            let pending_proposals = &self.pending_proposals;
            self.event_triggers.retain(|_event, proposal_set| {
                proposal_set.retain(|proposal| pending_proposals.contains_key(proposal));
                !proposal_set.is_empty()
            });
            self.num_triggers_registered_since_last_pruning = 0;
        }
    }

    fn on_message_received(&mut self, message: Message) {
        let mut proposals = Vec::new();
        for data in message.included_data() {
            let unvalidated_proposal = data.head_proposal;
            match unvalidated_proposal.validate_bounds(&self.session_boundaries) {
                Ok(proposal) => proposals.push(proposal),
                Err(error) => {
                    warn!(
                        target: LOG_TARGET,
                        "Message {:?} dropped as it contains \
                            proposal {:?} not within bounds ({:?}).",
                        message,
                        unvalidated_proposal,
                        error
                    );
                    return;
                }
            }
        }

        let mut message_info = PendingMessageInfo::new(message.clone());
        let message_id = self.assign_fresh_message_id();

        for proposal in proposals {
            self.add_message_proposal_dependency(&proposal, &mut message_info, message_id);
        }
        if message_info.pending_proposals.is_empty() {
            self.on_message_dependencies_resolved(message);
        } else {
            self.pending_messages.insert(message_id, message_info);
        }
    }
}

#[async_trait::async_trait]
impl<H, HB, BEV, RB, Message, R, V> Runnable for DataStore<H, HB, BEV, RB, Message, R, V>
where
    H: Header,
    HB: HeaderBackend<H>,
    BEV: BlockchainEvents<H> + Send + Sync + 'static,
    RB: RequestBlocks<H::Unverified>,
    Message: AlephNetworkMessage<H::Unverified>
        + std::fmt::Debug
        + Send
        + Sync
        + Clone
        + parity_scale_codec::Codec
        + 'static,
    R: Receiver<Message> + 'static,
    V: HeaderVerifier<H>,
{
    async fn run(mut self, exit: oneshot::Receiver<()>) {
        if let Err(err) = DataStore::run(&mut self, exit).await {
            error!(target: LOG_TARGET, "DataStore exited with error: {err}.");
        }
    }
}
