use clap::Parser;
use sp_core::{sr25519, Pair};
use std::fs;

/// Benchmarking tool expects to find key phrase files for the accounts
/// to send txs from under <BASE_PATH>/<ACCOUNT_ID>/<KEY_FILENAME>
#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    /// URL address(es) of the nodes to send transactions to
    #[clap(short, long, required = true)]
    pub nodes: Vec<String>,

    /// whether to use concurrency. If `true` one task is created per AccountId
    /// and the trasactions are spread uniformly over these tasks achieving higher throughpout
    #[clap(short, long)]
    pub parallel: bool,

    /// how many transactions send
    #[clap(short, long, default_value = "1000")]
    pub transactions: u64,

    /// root of the location where the directories with the account private keys are
    #[clap(short, long)]
    pub base_path: String,

    /// delimited collection of account ids
    #[clap(short, long, required = true)]
    pub account_ids: Vec<String>,

    /// filename where the secret phrase of the accounts is stored
    #[clap(short, long, default_value = "account_secret")]
    pub key_filename: String,
}

fn read_keypair(file: String) -> sr25519::Pair {
    let phrase = fs::read_to_string(&file)
        .unwrap_or_else(|_err| panic!("Could not read the phrase form the secret file: {}", file));
    sr25519::Pair::from_phrase(&phrase, None)
        .expect("not a secret phrase")
        .0
}

pub fn accounts(
    base_path: String,
    account_ids: Vec<String>,
    key_filename: String,
) -> Vec<sr25519::Pair> {
    account_ids
        .into_iter()
        .map(|id| {
            let file = format!("{}/{}/{}", &base_path, id, key_filename);
            read_keypair(file)
        })
        .collect::<Vec<sr25519::Pair>>()
}
