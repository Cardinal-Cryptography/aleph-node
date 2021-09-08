//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use aleph_primitives::AlephSessionApi;
use aleph_runtime::{self, opaque::Block, RuntimeApi};
use finality_aleph::{
    run_aleph_consensus, AlephBlockImport, AlephConfig, AuthorityId, AuthorityKeystore,
    JustificationNotification, Metrics, MillisecsPerBlock, SessionPeriod,
};
use futures::channel::mpsc;
use log::warn;
use sc_client_api::ExecutorProvider;
use sc_executor::native_executor_instance;
pub use sc_executor::NativeExecutor;
use sc_service::{error::Error as ServiceError, Configuration, TFullClient, TaskManager};
use sp_api::ProvideRuntimeApi;
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Zero},
};
use std::sync::Arc;

// Our native executor instance.
native_executor_instance!(
    pub Executor,
    aleph_runtime::api::dispatch,
    aleph_runtime::native_version,
);

type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

#[allow(clippy::type_complexity)]
pub fn new_partial(
    config: &Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sp_consensus::DefaultImportQueue<Block, FullClient>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            AlephBlockImport<Block, FullBackend, FullClient>,
            mpsc::UnboundedReceiver<JustificationNotification<Block>>,
            Option<Metrics<<Block as BlockT>::Header>>,
        ),
    >,
    ServiceError,
> {
    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, Executor>(config)?;

    let client: Arc<TFullClient<_, _, _>> = Arc::new(client);

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_handle(),
        client.clone(),
    );

    let metrics = config.prometheus_registry().cloned().and_then(|r| {
        Metrics::register(&r)
            .map_err(|_err| {
                warn!("Failed to register Prometheus metrics");
            })
            .ok()
    });

    let (justification_tx, justification_rx) = mpsc::unbounded();
    let aleph_block_import =
        AlephBlockImport::new(client.clone() as Arc<_>, justification_tx, metrics.clone());

    let inherent_data_providers = sp_inherents::InherentDataProviders::new();

    let aura_block_import = sc_consensus_aura::AuraBlockImport::<_, _, _, AuraPair>::new(
        aleph_block_import.clone(),
        client.clone(),
    );

    let import_queue = sc_consensus_aura::import_queue::<_, _, _, AuraPair, _, _>(
        sc_consensus_aura::slot_duration(&*client)?,
        aura_block_import,
        Some(Box::new(aleph_block_import.clone())),
        client.clone(),
        inherent_data_providers.clone(),
        &task_manager.spawn_handle(),
        config.prometheus_registry(),
        sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
    )?;

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        inherent_data_providers,
        other: (aleph_block_import, justification_rx, metrics),
    })
}

fn get_authority_id(keystore: SyncCryptoStorePtr) -> AuthorityId {
    SyncCryptoStore::ed25519_public_keys(&*keystore, finality_aleph::KEY_TYPE)[0].into()
}

/// Builds a new service for a full client.
pub fn new_full(mut config: Configuration) -> Result<TaskManager, ServiceError> {
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        inherent_data_providers,
        other: (block_import, justification_rx, metrics),
        ..
    } = new_partial(&config)?;

    config
        .network
        .extra_sets
        .push(finality_aleph::peers_set_config());

    let (network, network_status_sinks, system_rpc_tx, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            on_demand: None,
            block_announce_validator_builder: None,
        })?;

    let session_period = SessionPeriod(
        client
            .runtime_api()
            .session_period(&BlockId::Number(Zero::zero()))
            .unwrap(),
    );

    let millisecs_per_block = MillisecsPerBlock(
        client
            .runtime_api()
            .millisecs_per_block(&BlockId::Number(Zero::zero()))
            .unwrap(),
    );

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks: Option<()> = None;
    let prometheus_registry = config.prometheus_registry().cloned();
    let authority_id = get_authority_id(keystore_container.sync_keystore());

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();

        Box::new(move |deny_unsafe, _| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
                deny_unsafe,
            };

            crate::rpc::create_full(deps)
        })
    };

    let (_rpc_handlers, _telemetry_connection_notifier) =
        sc_service::spawn_tasks(sc_service::SpawnTasksParams {
            network: network.clone(),
            client: client.clone(),
            keystore: keystore_container.sync_keystore(),
            task_manager: &mut task_manager,
            transaction_pool: transaction_pool.clone(),
            rpc_extensions_builder,
            on_demand: None,
            remote_blockchain: None,
            backend,
            network_status_sinks,
            system_rpc_tx,
            config,
        })?;
    if role.is_authority() {
        let proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool,
            prometheus_registry.as_ref(),
        );

        let can_author_with =
            sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

        let aura = sc_consensus_aura::start_aura::<_, _, _, _, _, AuraPair, _, _, _, _>(
            sc_consensus_aura::slot_duration(&*client)?,
            client.clone(),
            select_chain.clone(),
            block_import,
            proposer,
            network.clone(),
            inherent_data_providers,
            force_authoring,
            backoff_authoring_blocks,
            keystore_container.sync_keystore(),
            can_author_with,
        )?;

        task_manager
            .spawn_essential_handle()
            .spawn_blocking("aura", aura);

        let aleph_config = AlephConfig {
            network,
            client,
            select_chain,
            session_period,
            millisecs_per_block,
            spawn_handle: task_manager.spawn_handle(),
            auth_keystore: AuthorityKeystore::new(authority_id, keystore_container.sync_keystore()),
            justification_rx,
            metrics,
        };
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("aleph", run_aleph_consensus(aleph_config));
    }

    network_starter.start_network();
    Ok(task_manager)
}
