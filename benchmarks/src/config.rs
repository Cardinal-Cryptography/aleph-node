use clap::Parser;
use sp_core::{sr25519, Pair};
use std::{fs, path::PathBuf};

//   let TOTAL_TRANSACTIONS = argv.total_transactions ? argv.total_transactions : 25000;
//   let TPS = argv.scale ? argv.scale : 100;
//   let TOTAL_THREADS = argv.total_threads ? argv.total_threads : 10;

//   let TOTAL_BATCHES = TOTAL_TRANSACTIONS / TPS;
//   let TRANSACTION_PER_BATCH = TPS / TOTAL_THREADS;
//   let TOTAL_USERS = TPS;

/// Benchmarking tool expects to find key phrase files for the accounts
/// to send txs from under <BASE_PATH>/<ACCOUNT_ID>/<KEY_FILENAME>
#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    /// URL address(es) of the nodes to send transactions to
    #[clap(long, default_value = "127.0.0.1:9943")]
    pub node_url: String,

    /// how many transactions to send
    #[clap(long, default_value = "100")]
    pub transactions: u64,

    /// what througput to use (transactions/s)
    #[clap(long, default_value = "10")]
    pub througput: u64,

    /// how many threads to create
    #[clap(long, default_value = "10")]
    pub threads: u64,

    /// secret phrase : a path to a file or passed on stdin
    #[clap(long, required = true)]
    pub phrase: String,
}

// TODO : read from file or stdin
// pub fn read_keypair(file: String) -> sr25519::Pair {
//     let phrase = fs::read_to_string(&file)
//         .unwrap_or_else(|_err| panic!("Could not read the phrase form the secret file: {}", file));
//     sr25519::Pair::from_phrase(&phrase, None)
//         .expect("not a secret phrase")
//         .0
// }

pub fn read_phrase(phrase: String) -> String {
    let file = PathBuf::from(&phrase);
    if file.is_file() {
        std::fs::read_to_string(phrase)
            .unwrap()
            .trim_end()
            .to_owned()
    } else {
        phrase.into()
    }
}

// pub fn accounts(
//     base_path: String,
//     account_ids: Vec<String>,
//     key_filename: String,
// ) -> Vec<sr25519::Pair> {
//     account_ids
//         .into_iter()
//         .map(|id| {
//             let file = format!("{}/{}/{}", &base_path, id, key_filename);
//             read_keypair(file)
//         })
//         .collect::<Vec<sr25519::Pair>>()
// }
