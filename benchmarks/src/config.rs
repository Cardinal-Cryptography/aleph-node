use clap::Parser;
use std::{fs, path::PathBuf};

/// Benchmarking tool expects to find key phrase files for the accounts
/// to send txs from under <BASE_PATH>/<ACCOUNT_ID>/<KEY_FILENAME>
#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    /// URL address(es) of the nodes to send transactions to
    #[clap(long, default_value = "127.0.0.1:9943")]
    pub nodes: Vec<String>,

    /// how many transactions to send
    #[clap(long, default_value = "10000")]
    pub transactions: u64,

    /// what throughput to use (transactions/s)
    #[clap(long, default_value = "1000")]
    pub throughput: u64,

    /// how many threads to create
    #[clap(long, default_value = "10")]
    pub threads: u64,

    /// secret phrase : a path to a file or passed on stdin
    #[clap(long, required = true)]
    pub phrase: String,
}

pub fn read_phrase(phrase: String) -> String {
    let file = PathBuf::from(&phrase);
    if file.is_file() {
        fs::read_to_string(phrase).unwrap().trim_end().to_owned()
    } else {
        phrase
    }
}
