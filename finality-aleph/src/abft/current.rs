use std::{sync::Arc, time::Duration};

use current_aleph_bft::{default_config, Config, DelayConfig, LocalIO, Terminator};
use log::debug;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block;

use crate::{
    abft::{common::exponential_slowdown, NetworkWrapper, SpawnHandleT},
    crypto::Signature,
    data_io::{AlephData, OrderedDataInterpreter},
    network::DataNetwork,
    oneshot,
    party::{
        backup::ABFTBackup,
        manager::{SubtaskCommon, Task},
    },
    CurrentNetworkData, Hasher, Keychain, NodeIndex, SessionId, SignatureSet, UnitCreationDelay,
};

pub fn run_member<
    B: Block,
    C: HeaderBackend<B> + Send + 'static,
    ADN: DataNetwork<CurrentNetworkData<B>> + 'static,
>(
    subtask_common: SubtaskCommon,
    multikeychain: Keychain,
    config: Config,
    network: NetworkWrapper<
        current_aleph_bft::NetworkData<Hasher, AlephData<B>, Signature, SignatureSet<Signature>>,
        ADN,
    >,
    data_provider: impl current_aleph_bft::DataProvider<AlephData<B>> + Send + 'static,
    ordered_data_interpreter: OrderedDataInterpreter<B, C>,
    backup: ABFTBackup,
) -> Task {
    let SubtaskCommon {
        spawn_handle,
        session_id,
    } = subtask_common;
    let (stop, exit) = oneshot::channel();
    let member_terminator = Terminator::create_root(exit, "member");
    let local_io = LocalIO::new(data_provider, ordered_data_interpreter, backup.0, backup.1);

    let task = {
        let spawn_handle = spawn_handle.clone();
        async move {
            debug!(target: "aleph-party", "Running the member task for {:?}", session_id);
            current_aleph_bft::run_session(
                config,
                local_io,
                network,
                multikeychain,
                spawn_handle,
                member_terminator,
            )
            .await;
            debug!(target: "aleph-party", "Member task stopped for {:?}", session_id);
        }
    };

    let handle = spawn_handle.spawn_essential("aleph/consensus_session_member", task);
    Task::new(handle, stop)
}

pub fn create_aleph_config(
    n_members: usize,
    node_id: NodeIndex,
    session_id: SessionId,
    unit_creation_delay: UnitCreationDelay,
) -> Config {
    let mut consensus_config =
        default_config(n_members.into(), node_id.into(), session_id.0 as u64);
    consensus_config.max_round = 7000;
    let unit_creation_delay = Arc::new(move |t| {
        if t == 0 {
            Duration::from_millis(2000)
        } else {
            exponential_slowdown(t, unit_creation_delay.0 as f64, 5000, 1.005)
        }
    });
    let delay_config = DelayConfig {
        tick_interval: Duration::from_millis(100),
        requests_interval: Duration::from_millis(3000),
        unit_rebroadcast_interval_min: Duration::from_millis(15000),
        unit_rebroadcast_interval_max: Duration::from_millis(20000),
        unit_creation_delay,
    };
    consensus_config.delay_config = delay_config;
    consensus_config
}
