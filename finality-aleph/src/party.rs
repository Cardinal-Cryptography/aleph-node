use crate::{
    data_io::DataIO,
    default_aleph_config,
    finalization::{
        chain_extension, finalize_block, finalize_block_as_authority, BlockSignatureAggregator,
    },
    justification::JustificationHandler,
    network,
    network::{split_network, ConsensusNetwork, NetworkData, SessionManager},
    AuthorityId, AuthorityKeystore, JustificationNotification, KeyBox, MultiKeychain, NodeIndex,
    SessionId, SpawnHandle,
};
use aleph_primitives::{AlephSessionApi, Session, ALEPH_ENGINE_ID};
use futures::{channel::mpsc, StreamExt};
use log::{debug, error, warn};
use parking_lot::Mutex;
use sc_client_api::backend::Backend;
use sc_service::SpawnTaskHandle;
use sp_api::{BlockId, NumberFor};
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

pub struct AlephParams<B: Block, N, C, SC> {
    pub config: crate::AlephConfig<B, N, C, SC>,
}

pub async fn run_consensus_party<B, N, C, BE, SC>(aleph_params: AlephParams<B, N, C, SC>)
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B, AuthorityId, NumberFor<B>>,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
    NumberFor<B>: Into<u32>,
{
    let AlephParams {
        config:
            crate::AlephConfig {
                network,
                client,
                select_chain,
                spawn_handle,
                auth_keystore,
                authority,
                justification_rx,
                period,
                ..
            },
    } = aleph_params;

    let sessions = Arc::new(Mutex::new(HashMap::new()));

    let handler_rx = run_justification_handler(
        &spawn_handle.clone().into(),
        justification_rx,
        sessions.clone(),
        auth_keystore.clone(),
        network.clone(),
        period,
    );
    let party = ConsensusParty::new(
        network,
        client,
        select_chain,
        spawn_handle,
        auth_keystore,
        authority,
        handler_rx,
        sessions.clone(),
    );

    debug!(target: "afa", "Consensus party has started.");
    party.run().await;
    error!(target: "afa", "Consensus party has finished unexpectedly.");
}

fn get_node_index(authorities: &[AuthorityId], my_id: &AuthorityId) -> Option<NodeIndex> {
    authorities
        .iter()
        .position(|a| a == my_id)
        .map(|id| id.into())
}

type SessionMap<Block> = HashMap<u32, Session<AuthorityId, NumberFor<Block>>>;

fn run_justification_handler<B: Block, N: network::Network<B> + 'static>(
    spawn_handle: &SpawnHandle,
    justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    sessions: Arc<Mutex<SessionMap<B>>>,
    auth_keystore: AuthorityKeystore,
    network: N,
    period: u32,
) -> mpsc::UnboundedReceiver<JustificationNotification<B>>
where
    NumberFor<B>: Into<u32>,
{
    let (finalization_proposals_tx, finalization_proposals_rx) = mpsc::unbounded();
    let handler = JustificationHandler::new(
        finalization_proposals_tx,
        justification_rx,
        sessions,
        auth_keystore,
        period,
        network,
    );

    debug!(target: "afa", "JustificationHandler started");
    spawn_handle
        .0
        .spawn("aleph/justification_handler", async move {
            handler.run().await;
        });

    finalization_proposals_rx
}

struct ConsensusParty<B, N, C, BE, SC>
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B, AuthorityId, NumberFor<B>>,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
    NumberFor<B>: From<u32>,
{
    network: N,
    sessions: Arc<Mutex<SessionMap<B>>>,
    spawn_handle: SpawnHandle,
    client: Arc<C>,
    select_chain: SC,
    auth_keystore: AuthorityKeystore,
    authority: AuthorityId,
    phantom: PhantomData<BE>,
    finalization_proposals_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
}

/// If we are on the authority list for the given session, runs an
/// AlephBFT task and returns `true` upon completion. Otherwise, immediately returns `false`.
#[allow(clippy::too_many_arguments)]
async fn maybe_run_session_as_authority<B, C, BE, SC>(
    authority: AuthorityId,
    auth_keystore: AuthorityKeystore,
    client: Arc<C>,
    session_manager: &SessionManager<NetworkData<B>>,
    session: Session<AuthorityId, NumberFor<B>>,
    spawn_handle: SpawnHandle,
    select_chain: SC,
    exit_rx: futures::channel::oneshot::Receiver<()>,
) -> bool
where
    B: Block,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B, AuthorityId, NumberFor<B>>,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
{
    let node_id = match get_node_index(&session.authorities, &authority) {
        Some(node_id) => node_id,
        None => {
            debug!(target: "afa", "Not an authority, thus not running a session");
            return false;
        }
    };
    debug!(target: "afa", "Running session #{}", session.session_id);
    let current_stop_h = session.stop_h;
    let (ordered_batch_tx, ordered_batch_rx) = mpsc::unbounded();

    let keybox = KeyBox {
        auth_keystore: auth_keystore.clone(),
        authorities: session.authorities.clone(),
        id: node_id,
    };
    let multikeychain = MultiKeychain::new(keybox);
    let session_id = SessionId(session.session_id as u64);

    let data_network = session_manager
        .start_session(session_id, multikeychain.clone())
        .await;

    let (aleph_network, rmc_network, forwarder) = split_network(data_network);

    spawn_handle.0.spawn("forward-data", forwarder);

    let consensus_config = default_aleph_config(
        session.authorities.len().into(),
        node_id,
        session_id.0 as u64,
    );
    let data_io = DataIO {
        select_chain: select_chain.clone(),
        ordered_batch_tx,
    };
    let aleph_task = {
        let multikeychain = multikeychain.clone();
        let spawn_handle = spawn_handle.clone();
        async move {
            let member =
                aleph_bft::Member::new(data_io, &multikeychain, consensus_config, spawn_handle);
            member.run_session(aleph_network, exit_rx).await;
            debug!(target: "afa", "Member for session #{} ended running", session_id.0);
        }
    };
    spawn_handle.0.spawn("aleph/consensus_session", aleph_task);

    debug!(target: "afa", "Consensus party #{} has started.", session_id.0);

    let mut aggregator = BlockSignatureAggregator::new(rmc_network, &multikeychain);

    let ordered_hashes = ordered_batch_rx.map(futures::stream::iter).flatten();
    let mut finalizable_chain = chain_extension(ordered_hashes, client.clone())
        .take_while(|header| std::future::ready(header.number() <= &current_stop_h))
        .fuse();

    loop {
        tokio::select! {
            header = finalizable_chain.next(), if !finalizable_chain.is_done() => {
                if let Some(header) = header {
                    aggregator.start_aggregation(header.hash()).await;
                    if *header.number() == current_stop_h {
                        aggregator.finish().await;
                    }
                } else {
                    debug!(target: "afa", "hashes to sign ended");
                }
            },
            multisigned_hash = aggregator.next_multisigned_hash() => {
                if let Some((hash, multisignature)) = multisigned_hash {
                    finalize_block_as_authority(client.clone(), hash, multisignature);
                } else {
                    debug!(target: "afa", "the stream of multisigned hashes has ended");
                    break;
                }
            },
        }
    }
    debug!(target: "afa", "Member for session {} ended", session_id.0);
    true
}

impl<B, N, C, BE, SC> ConsensusParty<B, N, C, BE, SC>
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B, AuthorityId, NumberFor<B>>,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
    NumberFor<B>: From<u32>,
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        network: N,
        client: Arc<C>,
        select_chain: SC,
        spawn_handle: SpawnTaskHandle,
        auth_keystore: AuthorityKeystore,
        authority: AuthorityId,
        finalization_proposals_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
        sessions: Arc<Mutex<SessionMap<B>>>,
    ) -> Self {
        Self {
            network,
            client,
            auth_keystore,
            select_chain,
            authority,
            finalization_proposals_rx,
            sessions,
            spawn_handle: spawn_handle.into(),
            phantom: PhantomData,
        }
    }
    async fn run_session(
        &mut self,
        session_manger: &SessionManager<NetworkData<B>>,
        session_id: u32,
    ) {
        let prev_block_number = match session_id.checked_sub(1) {
            None => 0.into(),
            Some(prev_id) => {
                self.sessions
                    .lock()
                    .get(&prev_id)
                    .expect("The current session should be known already")
                    .stop_h
            }
        };
        let session = match self
            .client
            .runtime_api()
            .current_session(&BlockId::Number(prev_block_number))
        {
            Ok(session) => {
                self.sessions.lock().insert(session_id, session.clone());
                session
            }
            _ => {
                error!(target: "afa", "No session found for current block #{}", 0);
                return;
            }
        };

        let proposals_task = {
            let client = self.client.clone();
            let current_stop_h = session.stop_h;
            let finalization_proposals_rx = &mut self.finalization_proposals_rx;
            async move {
                loop {
                    match finalization_proposals_rx.next().await {
                        Some(proposal) => {
                            finalize_block(
                                client.clone(),
                                proposal.hash,
                                proposal.number,
                                Some((ALEPH_ENGINE_ID, proposal.justification)),
                            );
                            if proposal.number == current_stop_h {
                                debug!(target: "afa", "finalized blocks up to #{}", current_stop_h);
                                return;
                            }
                        }
                        None => {
                            warn!(target: "afa", "the channel of block hashes to finalize ended too early");
                            return;
                        }
                    }
                }
            }
        };

        let (exit_tx, exit_rx) = futures::channel::oneshot::channel();

        // returns true if we participated in the session
        let session_task = maybe_run_session_as_authority(
            self.authority.clone(),
            self.auth_keystore.clone(),
            self.client.clone(),
            session_manger,
            session,
            self.spawn_handle.clone(),
            self.select_chain.clone(),
            exit_rx,
        );

        // We run concurrently `proposal_task` and `session_task` until either
        // * `proposal_tasks` terminates, or
        // * `session_task` terminates AND returns true.

        use futures::future::Either::*;
        futures::pin_mut!(proposals_task);
        futures::pin_mut!(session_task);
        // if session task terminates and we didn't participate as an authority, wait until we import blocks for the current session.
        if let Right((false, proposals_task)) =
            futures::future::select(proposals_task, session_task).await
        {
            proposals_task.await
        }
        if exit_tx.send(()).is_ok() {
            debug!(target: "afa", "terminating the member manually")
        }
        debug!(target: "afa", "session #{} of the party completed", session_id);
    }

    async fn run(mut self) {
        // Prepare and start the network
        let network = ConsensusNetwork::<NetworkData<B>, _, _>::new(
            self.network.clone(),
            "/cardinals/aleph/1".into(),
        );
        let session_manager = network.session_manager();

        let task = async move { network.run().await };
        self.spawn_handle.0.spawn("aleph/network", task);
        debug!(target: "afa", "Consensus network has started.");

        for curr_id in 0.. {
            self.run_session(&session_manager, curr_id).await
        }
    }
}

// TODO: :(
#[cfg(test)]
mod tests {}
