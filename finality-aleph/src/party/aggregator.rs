use crate::aggregator::SignableHash;
use crate::crypto::Signature;
use crate::{
    aggregator::BlockSignatureAggregator,
    crypto::KeyBox,
    data_io,
    data_io::AlephDataFor,
    finalization::should_finalize,
    justification::{AlephJustification, JustificationNotification},
    metrics::Checkpoint,
    network::{DataNetwork, RMCWrapper, RmcNetworkData},
    party::{AuthoritySubtaskCommon, Task},
    Metrics,
};
use aleph_bft::rmc::{DoublingDelayScheduler, ReliableMulticast};
use aleph_bft::{KeyBox as BftKeyBox, SignatureSet, SpawnHandle};
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use log::{debug, error, trace};
use sc_client_api::Backend;
use sp_api::NumberFor;
use sp_runtime::traits::{Block, Header};
use std::sync::Arc;

/// IO channels used by the aggregator task.
pub struct IO<B: Block> {
    pub ordered_units_from_aleph: mpsc::UnboundedReceiver<AlephDataFor<B>>,
    pub justifications_for_chain: mpsc::UnboundedSender<JustificationNotification<B>>,
}

async fn process_new_block_data<'a, B, C, N, BE>(
    aggregator: &mut BlockSignatureAggregator<
        'a,
        B::Hash,
        RmcNetworkData<B>,
        N,
        KeyBox,
        RMCWrapper<'a, SignableHash<B::Hash>, KeyBox>,
    >,
    new_block_data: data_io::AlephData<B::Hash, NumberFor<B>>,
    client: &Arc<C>,
    last_block_in_session: NumberFor<B>,
    mut last_block_seen: bool,
    mut last_finalized: B::Hash,
    metrics: &Option<Metrics<<B::Header as Header>::Hash>>,
) where
    B: Block,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    N: DataNetwork<RmcNetworkData<B>>,
    BE: Backend<B> + 'static,
    <B as Block>::Hash: AsRef<[u8]>,
{
    trace!(target: "aleph-party", "Received unit {:?} in aggregator.", new_block_data);
    if last_block_seen {
        return;
    }
    if let Some(metrics) = &metrics {
        metrics.report_block(
            new_block_data.hash,
            std::time::Instant::now(),
            Checkpoint::Ordered,
        );
    }
    if let Some(data) = should_finalize(
        last_finalized,
        new_block_data,
        client.as_ref(),
        last_block_in_session,
    ) {
        aggregator.start_aggregation(data.hash).await;
        last_finalized = data.hash;
        if data.number == last_block_in_session {
            aggregator.notify_last_hash();
            last_block_seen = true;
        }
    }
}

fn process_hash<B, C, BE>(
    hash: B::Hash,
    multisignature: SignatureSet<Signature>,
    justifications_for_chain: &mpsc::UnboundedSender<JustificationNotification<B>>,
    client: &Arc<C>,
) where
    B: Block,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    BE: Backend<B> + 'static,
{
    let number = client.number(hash).unwrap().unwrap();
    // The unwrap might actually fail if data availability is not implemented correctly.
    let notification = JustificationNotification {
        justification: AlephJustification {
            signature: multisignature,
        },
        hash,
        number,
    };
    if let Err(e) = justifications_for_chain.unbounded_send(notification) {
        error!(target: "aleph-party", "Issue with sending justification from Aggregator to JustificationHandler {:?}.", e);
    }
}

async fn run_aggregator<'a, B, C, N, BE>(
    mut aggregator: BlockSignatureAggregator<
        'a,
        B::Hash,
        RmcNetworkData<B>,
        N,
        KeyBox,
        RMCWrapper<'a, SignableHash<B::Hash>, KeyBox>,
    >,
    io: IO<B>,
    client: Arc<C>,
    last_block_in_session: NumberFor<B>,
    metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    mut exit_rx: oneshot::Receiver<()>,
) where
    B: Block,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    N: DataNetwork<RmcNetworkData<B>>,
    BE: Backend<B> + 'static,
    <B as Block>::Hash: AsRef<[u8]>,
{
    let IO {
        mut ordered_units_from_aleph,
        justifications_for_chain,
    } = io;
    let mut last_finalized = client.info().finalized_hash;
    let mut last_block_seen = false;
    loop {
        trace!(target: "aleph-party", "Aggregator Loop started a next iteration");
        tokio::select! {
            maybe_unit = ordered_units_from_aleph.next() => {
                if let Some(new_block_data) = maybe_unit {
                    process_new_block_data(
                        &mut aggregator,
                        new_block_data,
                        &client,
                        last_block_in_session,
                        last_block_seen,
                        last_finalized,
                        &metrics
                    ).await;
                } else {
                    debug!(target: "aleph-party", "Units ended in aggregator. Terminating.");
                    break;
                }
            }
            multisigned_hash = aggregator.next_multisigned_hash() => {
                if let Some((hash, multisignature)) = multisigned_hash {
                    process_hash(hash, multisignature, &justifications_for_chain, &client);
                } else {
                    debug!(target: "aleph-party", "The stream of multisigned hashes has ended. Terminating.");
                    return;
                }
            }
            _ = &mut exit_rx => {
                debug!(target: "aleph-party", "Aggregator received exit signal. Terminating.");
                return;
            }
        }
    }
    debug!(target: "aleph-party", "Aggregator awaiting an exit signal.");
    // this allows aggregator to exit after member,
    // otherwise it can exit too early and member complains about a channel to aggregator being closed
    let _ = exit_rx.await;
}

/// Runs the justification signature aggregator within a single session.
pub fn task<B, C, N, BE>(
    subtask_common: AuthoritySubtaskCommon,
    client: Arc<C>,
    io: IO<B>,
    last_block: NumberFor<B>,
    metrics: Option<Metrics<<B::Header as Header>::Hash>>,
    multikeychain: KeyBox,
    rmc_network: N,
) -> Task
where
    B: Block,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    N: DataNetwork<RmcNetworkData<B>> + 'static,
    BE: Backend<B> + 'static,
{
    let AuthoritySubtaskCommon {
        spawn_handle,
        session_id,
    } = subtask_common;
    let (stop, exit) = oneshot::channel();
    let task = {
        async move {
            let (messages_for_rmc, messages_from_network) = mpsc::unbounded();
            let (messages_for_network, messages_from_rmc) = mpsc::unbounded();
            let scheduler = DoublingDelayScheduler::new(tokio::time::Duration::from_millis(500));
            let rmc = ReliableMulticast::new(
                messages_from_network,
                messages_for_network,
                &multikeychain,
                multikeychain.node_count(),
                scheduler,
            );
            let aggregator = BlockSignatureAggregator::new(
                rmc_network,
                RMCWrapper::wrap(rmc),
                messages_for_rmc,
                messages_from_rmc,
                metrics.clone(),
            );
            debug!(target: "aleph-party", "Running the aggregator task for {:?}", session_id);
            run_aggregator(aggregator, io, client, last_block, metrics, exit).await;
            debug!(target: "aleph-party", "Aggregator task stopped for {:?}", session_id);
        }
    };

    let handle = spawn_handle.spawn_essential("aleph/consensus_session_aggregator", task);
    Task::new(handle, stop)
}
