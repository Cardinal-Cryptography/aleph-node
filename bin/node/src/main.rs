mod pruning_config;

use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use aleph_node::{new_authority, new_partial, Cli, Subcommand};
use log::info;
use primitives::HEAP_PAGES;
use pruning_config::PruningConfigValidator;
use sc_cli::{clap::Parser, SubstrateCli};
use sc_network::config::Role;
use sc_network_sync::service;
use sc_service::{Configuration, PartialComponents};
#[cfg(any(feature = "try-runtime", feature = "runtime-benchmarks"))]
use {aleph_node::ExecutorDispatch, aleph_runtime::Block, sc_executor::NativeExecutionDispatch};

#[cfg(feature = "runtime-benchmarks")]
use aleph_node::benchmarking::{inherent_benchmark_data, RemarkBuilder};

fn enforce_heap_pages(config: &mut Configuration) {
    config.default_heap_pages = Some(HEAP_PAGES);
}

fn main() -> sc_cli::Result<()> {
    let mut cli = Cli::parse();

    let pruning_config_validation_result = PruningConfigValidator::process(&mut cli);

    match &cli.subcommand {
        Some(Subcommand::BootstrapChain(cmd)) => cmd.run(),
        Some(Subcommand::BootstrapNode(cmd)) => cmd.run(),
        Some(Subcommand::ConvertChainspecToRaw(cmd)) => cmd.run(),
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    import_queue,
                    ..
                } = new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = new_partial(&config)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = new_partial(&config)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    import_queue,
                    ..
                } = new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.database))
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    backend,
                    ..
                } = new_partial(&config)?;
                Ok((cmd.run(client, backend, None), task_manager))
            })
        }
        #[cfg(feature = "try-runtime")]
        Some(Subcommand::TryRuntime(cmd)) => {
            use primitives::MILLISECS_PER_BLOCK;
            use sc_executor::{sp_wasm_interface::ExtendedHostFunctions, NativeExecutionDispatch};
            use try_runtime_cli::block_building_info::timestamp_with_aura_info;
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let registry = config.prometheus_config.as_ref().map(|cfg| &cfg.registry);
                let task_manager =
                    sc_service::TaskManager::new(config.tokio_handle.clone(), registry)
                        .map_err(|e| sc_cli::Error::Service(sc_service::Error::Prometheus(e)))?;

                Ok((
                    cmd.run::<Block, ExtendedHostFunctions<
                        sp_io::SubstrateHostFunctions,
                        <ExecutorDispatch as NativeExecutionDispatch>::ExtendHostFunctions,
                    >, _>(Some(timestamp_with_aura_info(
                        MILLISECS_PER_BLOCK,
                    ))),
                    task_manager,
                ))
            })
        }
        #[cfg(not(feature = "try-runtime"))]
        Some(Subcommand::TryRuntime) => Err("TryRuntime wasn't enabled when building the node. \
        You can enable it with `--features try-runtime`."
            .into()),
        #[cfg(feature = "runtime-benchmarks")]
        Some(Subcommand::Benchmark(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| {
                match cmd {
                    frame_benchmarking_cli::BenchmarkCmd::Pallet(cmd) => {
                        cmd.run::<Block, <ExecutorDispatch as NativeExecutionDispatch>::ExtendHostFunctions>(config)
                    }
                    frame_benchmarking_cli::BenchmarkCmd::Overhead(cmd) => {
                        let PartialComponents {
                            client,
                            ..
                        } = new_partial(&config)?;
                        let ext_builder = RemarkBuilder::new(client.clone());

                        cmd.run(
                            config,
                            client,
                            inherent_benchmark_data()?,
                            Vec::new(),
                            &ext_builder,
                        )
                    },
                    frame_benchmarking_cli::BenchmarkCmd::Machine(cmd) =>
                        cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()),
                    _ => {
                        Err(sc_cli::Error::Input("Wrong subcommand".to_string()))
                    }
                }
            })
        }
        #[cfg(not(feature = "runtime-benchmarks"))]
        Some(Subcommand::Benchmark) => Err(
            "Benchmarking wasn't enabled when building the node. You can enable it with \
                    `--features runtime-benchmarks`."
                .into(),
        ),
        None => {
            let runner = cli.create_runner(&cli.run)?;

            pruning_config_validation_result.report();

            let mut aleph_cli_config = cli.aleph;
            runner.run_node_until_exit(|mut config| async move {
                if matches!(config.role, Role::Full) {
                    if !aleph_cli_config.external_addresses().is_empty() {
                        panic!(
                            "A non-validator node cannot be run with external addresses specified."
                        );
                    }
                    // We ensure that external addresses for non-validator nodes are set, but to a
                    // value that is not routable. This will no longer be neccessary once we have
                    // proper support for non-validator nodes, but this requires a major
                    // refactor.
                    info!(
                        "Running as a non-validator node, setting dummy addressing configuration."
                    );
                    aleph_cli_config.set_dummy_external_addresses();
                }
                enforce_heap_pages(&mut config);
                new_authority(config, aleph_cli_config).map_err(sc_cli::Error::Service)
            })
        }
    }
}
