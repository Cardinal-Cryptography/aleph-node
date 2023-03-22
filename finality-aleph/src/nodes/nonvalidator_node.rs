use aleph_primitives::BlockNumber;
use log::{debug, error};
use sc_client_api::Backend;
use sc_network_common::ExHashT;
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};

use crate::{
    nodes::{setup_justification_handler, JustificationParams},
    session_map::{AuthorityProviderImpl, FinalityNotifierImpl, SessionMapUpdater},
    AlephConfig, BlockchainBackend,
    sync::Service as SyncService,
    finalization::AlephFinalizer,
    sync::{SubstrateFinalizationInfo, VerifierCache, SubstrateChainStatus, SubstrateChainStatusNotifier},
    network::{
        GossipService, SubstrateNetwork,
    },
};

pub async fn run_nonvalidator_node<B, H, C, BB, BE, SC>(aleph_config: AlephConfig<B, H, C, SC, BB>)
where
    B: Block,
    B::Header: Header<Number = BlockNumber>,
    H: ExHashT,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    BE: Backend<B> + 'static,
    BB: BlockchainBackend<B> + Send + 'static,
    SC: SelectChain<B> + 'static,
{
    let AlephConfig {
        network,
        client,
        blockchain_backend,
        select_chain,
        spawn_handle,
        keystore,
        metrics,
        unit_creation_delay,
        session_period,
        millisecs_per_block,
        justification_rx,
        backup_saving_path,
        external_addresses,
        validator_port,
        protocol_naming,
        ..
    } = aleph_config;
    let map_updater = SessionMapUpdater::new(
        AuthorityProviderImpl::new(client.clone()),
        FinalityNotifierImpl::new(client.clone()),
        session_period,
    );
    let session_authorities = map_updater.readonly_session_map();
    spawn_handle.spawn("aleph/updater", None, async move {
        debug!(target: "aleph-party", "SessionMapUpdater has started.");
        map_updater.run().await
    });

    const VERIFIER_CACHE_SIZE: usize = 43; // TODO - how much?

    let (gossip_network_service, authentication_network, block_sync_network) = GossipService::new(
        SubstrateNetwork::new(network.clone(), protocol_naming),
        spawn_handle.clone(),
    );
    let gossip_network_task = async move { gossip_network_service.run().await };

    let chain_events = SubstrateChainStatusNotifier::new(
        client.finality_notification_stream(),
        client.import_notification_stream(),
    );
    let chain_status = SubstrateChainStatus::new(client.clone());
    let verifier = VerifierCache::new(
        session_period,
        SubstrateFinalizationInfo::new(client.clone()),
        AuthorityProviderImpl::new(client.clone()),
        VERIFIER_CACHE_SIZE
    );
    let finalizer = AlephFinalizer::new(client.clone());
    let (sync_service, justifications_for_sync) = match SyncService::new(
        block_sync_network,
        chain_events,
        chain_status,
        verifier,
        finalizer,
        session_period,
        justification_rx,
    ) {
        Ok(x) => x,
        Err(e) => panic!("Failed to run Sync service: {}", e),
    };

    spawn_handle.spawn("aleph/gossip_network", None, gossip_network_task);
    debug!(target: "aleph-party", "Gossip network has started.");

    debug!(target: "aleph-party", "Sync has started.");
    sync_service.run().await;
    error!(target: "aleph-party", "Sync finished.");
}
