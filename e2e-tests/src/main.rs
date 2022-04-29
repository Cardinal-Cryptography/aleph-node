use std::{env, time::Instant};
use std::collections::HashMap;

use clap::Parser;
use aleph_e2e_client::{Config, TestCase, possible_test_cases};
use log::info;

fn main() -> anyhow::Result<()> {
    init_env();

    let config: Config = Config::parse();
    let test_cases = config.test_cases.clone();

    let possible_test_cases = possible_test_cases();
    // Possibility to handle specified vs. default test cases
    // is helpful to parallelize e2e tests.
    match test_cases {
        Some(cases) => {
            info!("Running specified test cases.");
            run_specified_test_cases(cases, possible_test_cases, &config)?;
        },
        None => {
            info!("Running default test cases.");
            run_default_test_cases(possible_test_cases, &config)?;
        }
    };
    Ok(())
}

fn init_env() {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "warn");
    }
    env_logger::init();
}

/// Runs default test cases in sequence.
fn run_default_test_cases(possible_test_cases: HashMap<&str, TestCase>, config: &Config) -> anyhow::Result<()> {
    let _ = possible_test_cases
        .iter()
        .map::<anyhow::Result<()>, _> (
            |(test_name, &test_case)| {
                run_test_case(test_case, test_name, config)?;
                Ok(())
            }
        ).collect::<Vec<_>>();
    Ok(())
}

/// Runs specified test cases in sequence.
/// Checks whether each provided test case is valid.
fn run_specified_test_cases(test_cases: Vec<String>, possible_test_cases: HashMap<&str, TestCase>, config: &Config) -> anyhow::Result<()> {
    let _ = test_cases
        .iter()
        .map::<anyhow::Result<()>, _>(
            |test_name| {
                if let Some(&test_case) = possible_test_cases.get(test_name.as_str()) {
                    run_test_case(test_case, test_name, config)?;
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        format!("Provided test case '{}' is not handled.", test_name)
                    ))
                }
            }
        ).collect::<Vec<_>>();
    Ok(())
}

/// Runs a particular test case. Handles different test case return types.
fn run_test_case(test_case: TestCase, test_name: &str, config: &Config) -> anyhow::Result<()> {
    match test_case {
        TestCase::BlockNumberResult(case) => {
            run(case, test_name, config)?;
        },
        TestCase::EmptyResult(case) => {
            run(case, test_name, config)?;
        },
    };
    Ok(())
}

/// Runs single test case. Allows for a generic return type.
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
