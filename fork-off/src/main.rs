use clap::Parser;
use common::{prefix_as_hex, read_file};
use serde_json::Value;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::io::Write;
use substrate_api_client::extrinsic::log::info;

#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    /// URL address of the node RPC endpoint for the chain you are forking
    #[clap(long, default_value = "http://127.0.0.1:9933")]
    pub http_rpc_endpoint: String,

    /// path to write the initial chainspec of the fork
    /// as generated with the `bootstrap-chain` command
    #[clap(long, default_value = "../docker/data/chainspec.json")]
    pub fork_spec_path: String,

    /// where to write the forked genesis chainspec
    #[clap(long, default_value = "../docker/data/chainspec.fork.json")]
    pub write_to_path: String,

    #[clap(long, default_value = "Aura, Aleph, Treasury, Vesting")]
    pub prefixes: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Config {
        http_rpc_endpoint,
        fork_spec_path,
        write_to_path,
        prefixes,
    } = Config::parse();

    env_logger::init();

    info!(
        "Running with config: \n\thttp_rpc_endpoint {}\n \tfork_spec_path: {}\n \twrite_to_path{}",
        http_rpc_endpoint, fork_spec_path, write_to_path
    );

    let mut fork_spec: Value = serde_json::from_str(&read_file(&fork_spec_path))?;

    // get current chain state (storage)
    let storage: Value = reqwest::Client::new()
        .post(http_rpc_endpoint)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "state_getPairs",
            "params": ["0x"]
        }))
        .send()
        .await
        .expect("Storage request has failed")
        .json()
        .await
        .expect("Could not deserialize response as JSON");

    let storage = storage["result"].as_array().expect("No result in response");

    info!("Succesfully retrieved chain state");

    // move the desired storage values from the snapshot of the chain to the forked chain genesis spec
    info!(
        "Following storage items will be moved to the fork: {:?}",
        prefixes
    );

    storage
        .iter()
        .filter(|pair| {
            prefixes.iter().any(|prefix| {
                let pair = pair.as_array().unwrap();
                let storage_key = pair[0].as_str().unwrap();
                storage_key.starts_with(&format!("0x{}", prefix_as_hex(prefix)))
                    || storage_key.eq("0x3a636f6465") // code
            })
        })
        .for_each(|pair| {
            let pair = pair.as_array().unwrap();
            let k = &pair[0].as_str().unwrap();
            let v = &pair[1];
            info!("Moving {} to the fork", k);
            fork_spec["genesis"]["raw"]["top"][k] = v.to_owned();
        });

    // write out the fork spec
    let json = serde_json::to_string(&fork_spec)?;
    info!("Writing forked chain spec to {}", &write_to_path);
    write_to_file(write_to_path, json.as_bytes());

    info!("Done!");
    Ok(())
}

pub fn write_to_file(write_to_path: String, data: &[u8]) {
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&write_to_path)
    {
        Ok(file) => file,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => match File::create(&write_to_path) {
                Ok(file) => file,
                Err(why) => panic!("Cannot create file: {:?}", why),
            },
            _ => panic!("Unexpected error when creating file: {}", &write_to_path),
        },
    };

    file.write_all(data).expect("Could not write to file");
}
