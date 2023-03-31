use std::{collections::HashSet, fmt::Debug, iter, time::Duration};

use futures::{channel::mpsc, StreamExt};
use log::{debug, error, info, warn};
use tokio::time::interval;

use crate::{
    network::GossipNetwork,
    sync::{
        data::{
            BranchKnowledge, NetworkData, Request, State, VersionWrapper, VersionedNetworkData,
        },
        forest::Interest,
        handler::{Error as HandlerError, Handler, SyncAction},
        task_queue::TaskQueue,
        ticker::Ticker,
        BlockIdFor, BlockIdentifier, ChainStatus, ChainStatusNotification, ChainStatusNotifier,
        Finalizer, Header, Justification, JustificationSubmissions, Verifier, LOG_TARGET,
    },
    SessionPeriod,
};

const BROADCAST_COOLDOWN: Duration = Duration::from_millis(200);
const BROADCAST_PERIOD: Duration = Duration::from_secs(1);
// TODO: Remove after finishing the sync rewrite.
const FINALIZATION_STALL_CHECK_PERIOD: Duration = Duration::from_secs(30);

/// A service synchronizing the knowledge about the chain between the nodes.
pub struct Service<
    J: Justification + Debug,
    N: GossipNetwork<VersionedNetworkData<J>>,
    CE: ChainStatusNotifier<J::Header>,
    CS: ChainStatus<J>,
    V: Verifier<J>,
    F: Finalizer<J>,
> {
    network: VersionWrapper<J, N>,
    handler: Handler<N::PeerId, J, CS, V, F>,
    tasks: TaskQueue<BlockIdFor<J>>,
    broadcast_ticker: Ticker,
    chain_events: CE,
    justifications_from_user: mpsc::UnboundedReceiver<J::Unverified>,
    additional_justifications_from_user: mpsc::UnboundedReceiver<J::Unverified>,
}

impl<J: Justification> JustificationSubmissions<J> for mpsc::UnboundedSender<J::Unverified> {
    type Error = mpsc::TrySendError<J::Unverified>;

    fn submit(&mut self, justification: J::Unverified) -> Result<(), Self::Error> {
        self.unbounded_send(justification)
    }
}

impl<
        J: Justification + Debug,
        N: GossipNetwork<VersionedNetworkData<J>>,
        CE: ChainStatusNotifier<J::Header>,
        CS: ChainStatus<J>,
        V: Verifier<J>,
        F: Finalizer<J>,
    > Service<J, N, CE, CS, V, F>
{
    /// Create a new service using the provided network for communication. Also returns an
    /// interface for submitting additional justifications.
    pub fn new(
        network: N,
        chain_events: CE,
        chain_status: CS,
        verifier: V,
        finalizer: F,
        period: SessionPeriod,
        additional_justifications_from_user: mpsc::UnboundedReceiver<J::Unverified>,
    ) -> Result<(Self, impl JustificationSubmissions<J> + Clone), HandlerError<J, CS, V, F>> {
        let network = VersionWrapper::new(network);
        let handler = Handler::new(chain_status, verifier, finalizer, period)?;
        let tasks = TaskQueue::new();
        let broadcast_ticker = Ticker::new(BROADCAST_PERIOD, BROADCAST_COOLDOWN);
        let (justifications_for_sync, justifications_from_user) = mpsc::unbounded();
        Ok((
            Service {
                network,
                handler,
                tasks,
                broadcast_ticker,
                chain_events,
                justifications_from_user,
                additional_justifications_from_user,
            },
            justifications_for_sync,
        ))
    }

    fn backup_request(&mut self, block_id: BlockIdFor<J>) {
        info!(target: LOG_TARGET, "Sync Service::backup_request");
        self.tasks.schedule_in(block_id, Duration::from_secs(5));
    }

    fn delayed_request(&mut self, block_id: BlockIdFor<J>) {
        info!(target: LOG_TARGET, "Sync Service::delayed_request");
        self.tasks.schedule_in(block_id, Duration::from_millis(500));
    }

    fn request(&mut self, block_id: BlockIdFor<J>) {
        info!(target: LOG_TARGET, "Sync Service::request");
        self.tasks.schedule_in(block_id, Duration::ZERO);
    }

    fn broadcast(&mut self) {
        info!(target: LOG_TARGET, "Sync Service::broadcast");
        let state = match self.handler.state() {
            Ok(state) => state,
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Failed to construct own knowledge state: {}.", e
                );
                return;
            }
        };
        let data = NetworkData::StateBroadcast(state);
        if let Err(e) = self.network.broadcast(data) {
            warn!(target: LOG_TARGET, "Error sending broadcast: {}.", e);
        }
    }

    fn send_request_for(
        &mut self,
        block_id: BlockIdFor<J>,
        branch_knowledge: BranchKnowledge<J>,
        peers: HashSet<N::PeerId>,
    ) {
        info!(target: LOG_TARGET, "Sync Service::send_request_for");
        let state = match self.handler.state() {
            Ok(state) => state,
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Failed to construct own knowledge state: {}.", e
                );
                return;
            }
        };
        let request = Request::new(block_id, branch_knowledge, state);
        let data = NetworkData::Request(request);
        if let Err(e) = self.network.send_to_random(data, peers) {
            warn!(target: LOG_TARGET, "Error sending request: {}.", e);
        }
    }

    fn send_to(&mut self, data: NetworkData<J>, peer: N::PeerId) {
        use NetworkData::*;
        info!(target: LOG_TARGET, "Sync Service::send_to");
        match data.clone() {
            StateBroadcast(state) => info!(
                target: LOG_TARGET,
                "Sync Service::send_to StateBroadcast top justification id {:?}",
                state.top_justification().id()
            ),
            StateBroadcastResponse(j1, j2) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::send_to StateBroadcastResponse j1 {:?} j2 {:?}",
                    j1.id(),
                    j2.map(|x| x.id())
                );
            }
            Request(request) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::send_to Request {:?}", request
                );
            }
            RequestResponse(response) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::send_to Response {:?}", response
                );
            }
        }
        if let Err(e) = self.network.send_to(data, peer) {
            warn!(target: LOG_TARGET, "Error sending response: {}.", e);
        }
    }

    fn perform_sync_action(&mut self, action: SyncAction<J>, peer: N::PeerId) {
        use SyncAction::*;
        info!(target: LOG_TARGET, "Sync Service::perform_sync_action");
        match action {
            Response(data) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::perform_sync_action - respond to peer {:?}", peer
                );
                self.send_to(data, peer);
            }
            Task(block_id) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::perform_sync_action - request block id {:?}", block_id
                );
                self.request(block_id);
            }
            Noop => info!(target: LOG_TARGET, "Sync Service::perform_sync_action NOOP"),
        }
    }

    fn handle_state(&mut self, state: State<J>, peer: N::PeerId) {
        info!(target: LOG_TARGET, "Sync Service::handle_state");
        match self.handler.handle_state(state, peer.clone()) {
            Ok(action) => self.perform_sync_action(action, peer),
            Err(e) => warn!(
                target: LOG_TARGET,
                "Error handling sync state from {:?}: {}.", peer, e
            ),
        }
    }

    fn handle_justifications(
        &mut self,
        justifications: Vec<J::Unverified>,
        peer: Option<N::PeerId>,
    ) {
        info!(target: LOG_TARGET, "Sync Service::handle_justifications");
        let mut previous_block_id = None;
        for justification in justifications {
            info!(
                target: LOG_TARGET,
                "Sync Service::handle_justifications handling justification {:?}", justification
            );
            let maybe_block_id = match self
                .handler
                .handle_justification(justification, peer.clone())
            {
                Ok(maybe_id) => maybe_id,
                Err(e) => {
                    warn!(
                        target: LOG_TARGET,
                        "Error handling justification from {:?}: {}.", peer, e
                    );
                    return;
                }
            };
            info!(
                target: LOG_TARGET,
                "Sync Service::handle_justifications handled justification, maybe_block_id is {:?}",
                maybe_block_id
            );
            if let Some(block_id) = maybe_block_id {
                if let Some(previous_block_id) = previous_block_id {
                    self.backup_request(previous_block_id);
                }
                previous_block_id = Some(block_id);
            }
            info!(
                target: LOG_TARGET,
                "Sync Service::handle_justifications next step, previous_block_id is {:?}",
                previous_block_id
            );
        }
        info!(
            target: LOG_TARGET,
            "Sync Service::handle_justifications last if, previous_block_id is {:?}",
            previous_block_id
        );
        if let Some(block_id) = previous_block_id {
            self.request(block_id);
        }
    }

    fn handle_request(&mut self, request: Request<J>, peer: N::PeerId) {
        info!(target: LOG_TARGET, "Sync Service::handle_request");
        match self.handler.handle_request(request) {
            Ok(action) => self.perform_sync_action(action, peer),
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Error handling request from {:?}: {}.", peer, e
                );
            }
        }
    }

    fn handle_network_data(&mut self, data: NetworkData<J>, peer: N::PeerId) {
        use NetworkData::*;
        info!(target: LOG_TARGET, "Sync Service::handle_network_data");
        match data.clone() {
            StateBroadcast(state) => info!(
                target: LOG_TARGET,
                "Sync Service::handle_network_data StateBroadcast top justification id {:?}",
                state.top_justification().id()
            ),
            StateBroadcastResponse(j1, j2) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::handle_network_data StateBroadcastResponse j1 {:?} j2 {:?}",
                    j1.id(),
                    j2.map(|x| x.id())
                );
            }
            Request(request) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::handle_network_data Request {:?}", request
                );
            }
            RequestResponse(response) => {
                info!(
                    target: LOG_TARGET,
                    "Sync Service::handle_network_data Response {:?}", response
                );
            }
        }

        match data {
            StateBroadcast(state) => self.handle_state(state, peer),
            StateBroadcastResponse(justification, maybe_justification) => {
                self.handle_justifications(
                    iter::once(justification)
                        .chain(maybe_justification)
                        .collect(),
                    Some(peer),
                );
            }
            Request(request) => {
                let state = request.state().clone();
                self.handle_request(request, peer.clone());
                self.handle_state(state, peer)
            }
            RequestResponse(justifications) => {
                self.handle_justifications(justifications, Some(peer))
            }
        }
    }

    fn handle_task(&mut self, block_id: BlockIdFor<J>) {
        use Interest::*;
        info!(target: LOG_TARGET, "Sync Service::handle_task");
        match self.handler.block_state(&block_id) {
            TopRequired {
                know_most,
                branch_knowledge,
            } => {
                self.send_request_for(block_id.clone(), branch_knowledge, know_most);
                self.delayed_request(block_id);
            }
            Required {
                know_most,
                branch_knowledge,
            } => {
                self.send_request_for(block_id.clone(), branch_knowledge, know_most);
                self.backup_request(block_id);
            }
            Uninterested => (),
        }
    }

    fn handle_chain_event(&mut self, event: ChainStatusNotification<J::Header>) {
        use ChainStatusNotification::*;
        info!(target: LOG_TARGET, "Sync Service::handle_chain_event");
        match event {
            BlockImported(header) => {
                if let Err(e) = self.handler.block_imported(header) {
                    error!(
                        target: LOG_TARGET,
                        "Error marking block as imported: {}.", e
                    );
                }
            }
            BlockFinalized(_) => {
                if self.broadcast_ticker.try_tick() {
                    self.broadcast();
                }
            }
        }
    }

    /// Stay synchronized.
    pub async fn run(mut self) {
        // TODO: Remove after finishing the sync rewrite.
        let mut stall_ticker = interval(FINALIZATION_STALL_CHECK_PERIOD);
        let mut last_top_number = 0;
        loop {
            tokio::select! {

                maybe_data = self.network.next() => {
                    info!(target: LOG_TARGET, "Sync Service::run self.network.next()");
                    match maybe_data {
                        Ok((data, peer)) => self.handle_network_data(data, peer),
                        Err(e) => warn!(target: LOG_TARGET, "Error receiving data from network: {}.", e),
                    };
                },

                Some(block_id) = self.tasks.pop() => {
                    info!(target: LOG_TARGET, "Sync Service::run self.tasks.pop()");
                    self.handle_task(block_id);
                },

                _ = self.broadcast_ticker.wait_and_tick() => {
                    info!(target: LOG_TARGET, "Sync Service::run self.broadcast_ticker.wait_and_tick()");
                    self.broadcast();
                },

                maybe_event = self.chain_events.next() => {
                    info!(target: LOG_TARGET, "Sync Service::run self.chain_events.next()");
                    match maybe_event {
                        Ok(chain_event) => self.handle_chain_event(chain_event),
                        Err(e) => warn!(target: LOG_TARGET, "Error when receiving a chain event: {}.", e),
                    };
                },

                maybe_justification = self.justifications_from_user.next() => {
                    info!(target: LOG_TARGET, "Sync Service::run self.justifications_from_user.next()");
                    match maybe_justification {
                        // The block will be imported independently due to `JustificationSubmissions` requirements.
                        // Therefore, we do not create a task requesting the block from peers.
                        Some(justification) => if let Err(e) = self.handler.handle_justification(justification, None) {
                            warn!(
                                target: LOG_TARGET,
                                "Error while handling justification from user: {}.", e
                            );
                        },
                        None => warn!(target: LOG_TARGET, "Channel with justifications from user closed."),
                    };
                },

                maybe_justification = self.additional_justifications_from_user.next() => {
                    info!(target: LOG_TARGET, "Sync Service::run self.additional_justifications_from_user.next()");
                    match maybe_justification {
                        Some(justification) => self.handle_justifications(Vec::from([justification]), None),
                        None => warn!(target: LOG_TARGET, "Channel with justifications from user closed."),
                    };
                },

                // _ = stall_ticker.tick() => {
                //     match self.handler.state() {
                //         Ok(state) => {
                //             let top_number = state.top_justification().id().number();
                //             if top_number == last_top_number {
                //                 error!(
                //                     target: LOG_TARGET,
                //                     "Sync stall detected, recreating the Forest."
                //                 );
                //                 if let Err(e) = self.handler.refresh_forest() {
                //                     error!(
                //                         target: LOG_TARGET,
                //                         "Error when recreating the Forest: {}.", e
                //                     );
                //                 }
                //                 last_top_number = 0;
                //             } else {
                //                 last_top_number = top_number;
                //             }
                //         },
                //         Err(e) => error!(
                //                         target: LOG_TARGET,
                //                         "Error when retrieving Handler state: {}.", e
                //                     ),
                //     }
                // }
            }
        }
    }
}
