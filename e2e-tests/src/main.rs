use std::{env, time::Instant};

use clap::Parser;

use aleph_client::{create_connection, print_storages};
use aleph_e2e_client::{config::Config, test};
use log::info;

fn main() -> anyhow::Result<()> {
    init_env();

    let config: Config = Config::parse();

    if config.storage_debug {
        let connection = create_connection(&config.node, config.protocol);
        print_storages(&connection);
        return Ok(());
    }

    run_tests(config)
}

fn run_tests(config: Config) -> anyhow::Result<()> {
    run(test::finalization, "finalization", &config)?;
    run(test::token_transfer, "token transfer", &config)?;
    run(test::channeling_fee, "channeling fee", &config)?;
    run(test::treasury_access, "treasury access", &config)?;
    run(test::batch_transactions, "batch_transactions", &config)?;
    run(test::staking_era_payouts, "staking_era_payouts", &config)?;
    run(
        test::staking_new_validator,
        "staking_new_validator",
        &config,
    )?;
    run(test::change_validators, "validators change", &config)?;
    run(test::fee_calculation, "fee calculation", &config)?;

    Ok(())
}

fn init_env() {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "warn");
    }
    env_logger::init();
}

fn run<T>(
    testcase: fn(&Config) -> anyhow::Result<T>,
    name: &str,
    config: &Config,
) -> anyhow::Result<()> {
    info!("Running test: {}", name);
    let start = Instant::now();
    testcase(config).map(|_| {
        let elapsed = Instant::now().duration_since(start);
        println!("Ok! Elapsed time {}ms", elapsed.as_millis());
    })
}
