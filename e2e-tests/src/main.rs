use std::env;
use std::time::Instant;

use clap::Parser;

use config::Config;

use crate::finalization::test_finalization;
use crate::transfer::{test_fee_calculation, test_token_transfer};
use crate::treasury::{test_channeling_fee, test_treasury_access};
use crate::validators_change::test_change_validators;

mod config;
mod finalization;
mod transfer;
mod treasury;
mod utils;
mod validators_change;

fn main() -> anyhow::Result<()> {
    init_env();

    let config: Config = Config::parse();

    run(test_finalization, "finalization", config.clone())?;
    run(test_fee_calculation, "fee calculation", config.clone())?;
    run(test_token_transfer, "token transfer", config.clone())?;
    run(test_channeling_fee, "channeling fee", config.clone())?;
    run(test_treasury_access, "treasury access", config.clone())?;
    run(test_change_validators, "validators change", config)?;

    Ok(())
}

fn init_env() {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "warn");
    }
    env_logger::init();
}

fn run<T>(
    testcase: fn(Config) -> anyhow::Result<T>,
    name: &str,
    config: Config,
) -> anyhow::Result<()> {
    println!("Running test: {}", name);
    let start = Instant::now();
    testcase(config).map(|_| {
        let elapsed = Instant::now().duration_since(start);
        println!("Ok! Elapsed time {}ms", elapsed.as_millis());
    })
}
