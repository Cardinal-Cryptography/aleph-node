use crate::{
    data_io::{BlockFinalizer, DataIO, ProposalSelect},
    finalization::{finalize_block, finalize_block_as_authority},
    hash,
    justification::JustificationHandler,
    network,
    network::{ConsensusNetwork, SessionManagar},
    AuthorityId, AuthorityKeystore, ConsensusConfig, JustificationNotification, KeyBox, NodeIndex,
    SessionId, SpawnHandle,
};
use aleph_primitives::{Session, ALEPH_ENGINE_ID};
use futures::{channel::mpsc, select, StreamExt};
use log::{debug, error};
use sc_client_api::backend::Backend;
use sp_api::NumberFor;
use sp_consensus::SelectChain;
use sp_runtime::traits::{BlakeTwo256, Block};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

pub struct AlephParams<B: Block, N, C, SC> {
    pub config: crate::AlephConfig<B, N, C, SC>,
}

pub async fn run_consensus_party<B, N, C, BE, SC>(aleph_params: AlephParams<B, N, C, SC>)
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
{
    let x = aleph_params.config.authorities.clone();
    let party = ConsensusParty::new(aleph_params);

    debug!(target: "afa", "Consensus party has started.");
    // TODO: remove x when the information from pallet will be available
    party.run(x).await;
    error!(target: "afa", "Consensus party has finished unexpectedly.");
}

struct SessionInstance<B>
where
    B: Block,
{
    pub(crate) session: Session<AuthorityId, NumberFor<B>>,
    pub(crate) exit_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

struct ConsensusParty<B, N, C, BE, SC>
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
{
    network: N,
    sessions: HashMap<u64, SessionInstance<B>>,
    spawn_handle: SpawnHandle,
    client: Arc<C>,
    select_chain: SC,
    auth_keystore: AuthorityKeystore,
    authority: AuthorityId,
    phantom: PhantomData<BE>,
    finalization_proposals_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
}

pub trait NumberOps:
    std::ops::Add<Output = Self>
    + std::ops::Sub<Output = Self>
    + std::ops::Mul<Output = Self>
    + From<u32>
{
}

impl<T> NumberOps for T
where
    T: std::ops::Add<Output = Self>,
    T: std::ops::Sub<Output = Self>,
    T: std::ops::Mul<Output = Self>,
    T: From<u32>,
{
}

impl<B, N, C, BE, SC> ConsensusParty<B, N, C, BE, SC>
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
    NumberFor<B>: NumberOps,
{
    pub(crate) fn new(aleph_params: AlephParams<B, N, C, SC>) -> Self {
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
                    ..
                },
        } = aleph_params;

        let spawn_handle = spawn_handle.into();
        let finalization_proposals_rx = Self::run_handler(&spawn_handle, justification_rx);

        Self {
            network,
            client,
            auth_keystore,
            select_chain,
            authority,
            spawn_handle,
            finalization_proposals_rx,
            sessions: HashMap::new(),
            phantom: PhantomData,
        }
    }

    fn get_node_index(authorities: &[AuthorityId], my_id: &AuthorityId) -> Option<NodeIndex> {
        authorities
            .iter()
            .position(|a| a == my_id)
            .map(|id| id.into())
    }

    fn create_session(
        authority: AuthorityId,
        auth_keystore: AuthorityKeystore,
        select_chain: SC,
        client: Arc<C>,
        spawn_handle: SpawnHandle,
        session: Session<AuthorityId, NumberFor<B>>,
        session_manager: &SessionManagar,
    ) -> Option<(SessionInstance<B>, mpsc::UnboundedReceiver<B::Hash>)> {
        // If we are in session authorities run consensus.
        if let Some(node_id) = Self::get_node_index(&session.authorities, &authority) {
            let (ordered_batch_tx, ordered_batch_rx) = mpsc::unbounded();
            let (proposition_tx, proposition_rx) = mpsc::unbounded();
            let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();

            let block_finalizer = BlockFinalizer::new(client, ordered_batch_rx, proposition_tx);
            let task = async move { block_finalizer.run().await };
            spawn_handle.0.spawn("aleph/finalizer", task);
            debug!(target: "afa", "Block finalizer has started.");

            let session_network = session_manager
                .start_session(SessionId(session.session_id), session.authorities.clone());

            let data_io = DataIO {
                select_chain,
                ordered_batch_tx,
            };

            let consensus_config = ConsensusConfig {
                node_id,
                session_id: session.session_id,
                n_members: session.authorities.len().into(),
                create_lag: std::time::Duration::from_millis(500),
            };

            let spawn_clone = spawn_handle.clone();
            let authorities = session.authorities.clone();

            let task = async move {
                let keybox = KeyBox {
                    auth_keystore,
                    authorities,
                    id: node_id,
                };
                let member = rush::Member::<hash::Wrapper<BlakeTwo256>, _, _, _, _>::new(
                    data_io,
                    &keybox,
                    session_network,
                    consensus_config,
                );
                member.run_session(spawn_clone, exit_rx).await;
            };

            spawn_handle.0.spawn("aleph/consensus_session", task);
            debug!(target: "afa", "Consensus party #{} has started.", session.session_id);

            return Some((
                SessionInstance {
                    session,
                    exit_tx: Some(exit_tx),
                },
                proposition_rx,
            ));
        }

        None
    }

    fn run_handler(
        spawn_handler: &SpawnHandle,
        justification_rx: mpsc::UnboundedReceiver<JustificationNotification<B>>,
    ) -> mpsc::UnboundedReceiver<JustificationNotification<B>> {
        let (finalization_proposals_tx, finalization_proposals_rx) = mpsc::unbounded();
        let handler = JustificationHandler::new(finalization_proposals_tx, justification_rx);

        debug!(target: "afa", "JustificationHandler started");
        spawn_handler
            .0
            .spawn("aleph/justification_handler", async move {
                handler.run().await;
            });

        finalization_proposals_rx
    }

    async fn run(mut self, authorities: Vec<AuthorityId>) {
        // Prepare and start the network
        let network = ConsensusNetwork::new(self.network, "/cardinals/aleph/1");
        let session_manager = network.session_manager();

        let task = async move { network.run().await };
        self.spawn_handle.0.spawn("aleph/network", task);
        debug!(target: "afa", "Consensus network has started.");

        let (new_receivers_tx, new_receivers_rx) = mpsc::unbounded();
        let mut proposition_select = ProposalSelect::<B>::new(new_receivers_rx).fuse();

        for curr_id in 0.. {
            // TODO: Ask runtime for current Session and future sessions and run/store them.
            let current_session: Session<AuthorityId, NumberFor<B>> = Session {
                session_id: curr_id,
                stop_h: ((curr_id as u32 + 1) * 100).into(),
                start_h: 0.into(),
                authorities: authorities.clone(),
            };

            // Start new session if we are in the authority set.
            let current_stop_h = current_session.stop_h;
            if let Some((instance, proposition_rx)) = Self::create_session(
                self.authority.clone(),
                self.auth_keystore.clone(),
                self.select_chain.clone(),
                self.client.clone(),
                self.spawn_handle.clone(),
                current_session,
                &session_manager,
            ) {
                self.sessions.insert(curr_id, instance);
                new_receivers_tx
                    .unbounded_send((curr_id, proposition_rx))
                    .expect("Sending channel should succeed");
            }

            // TODO: handle waiting blocks
            ;
            while self.client.info().finalized_number != current_stop_h {
                select! {
                    x = proposition_select.next() => {
                        if let Some((id, hash)) = x {
                            if id == curr_id {
                                // TODO: given current session and authorities finalize block
                                finalize_block_as_authority(&self.client, hash, &self.auth_keystore);
                            } else {
                                // TODO: add to queue to process when session with `id` will start.
                            }
                        }
                    },
                    x = self.finalization_proposals_rx.next() => {
                        if let Some(proposal) = x {
                            // TODO: check if we should do this
                            finalize_block(self.client.clone(), proposal.hash, proposal.number, Some((
                                ALEPH_ENGINE_ID,
                                proposal.justification
                            )));
                        }
                    },
                    complete => {
                        error!(target: "afa", "Proposal channel and proposition_select channels finished");

                        // if this condition is false no hopes for restarting.
                        if self.client.info().finalized_number != current_stop_h {
                            return;
                        }
                    }
                }
            }

            if let Some(instance) = self.sessions.remove(&curr_id) {
                if let Some(exit_tx) = instance.exit_tx {
                    // Signal the end of the session
                    debug!(target: "afa", "Signaling end of the consensus party #{}.", curr_id);
                    exit_tx.send(()).expect("Closing member session");
                }
                self.sessions.insert(
                    curr_id,
                    SessionInstance {
                        session: instance.session,
                        exit_tx: None,
                    },
                );
            }

            debug!(target: "afa", "Moving to new session #{}.", curr_id + 1);
        }
    }
}
