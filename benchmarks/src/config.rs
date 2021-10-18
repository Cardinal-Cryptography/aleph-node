use clap::Parser;
use sp_core::{sr25519, Pair};
use std::fs;

#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    // #[clap(short, long, default_value = "http")]
    // protocol: String,
    #[clap(short, long, default_value = "127.0.0.1")]
    pub host: String,

    #[clap(short, long, default_value = "9943")]
    pub port: u32,

    /// how many concurrent tasks to spawn. Requests are spread over these connections    
    #[clap(short, long, default_value = "2")]
    pub concurrency: usize,

    /// how many transactions / s to send
    #[clap(short, long, default_value = "100")]
    pub throughput: u64,

    /// how long to run the benchmark for (in seconds)
    #[clap(short, long, default_value = "10")]
    pub duration: u64,

    /// root of the location where the keys are    
    #[clap(short, long)]
    pub base_path: String,

    /// delimited collection of account ids
    #[clap(short, long)]
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
    account_ids: Vec<String>,
    base_path: String,
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
