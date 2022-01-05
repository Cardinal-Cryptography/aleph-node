use common::{create_connection, prefix_as_hex, read_file, storage_key, storage_key_hash};
use reqwest;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::format;
use std::fs::File;
use std::process::Command;
use std::str;

// TODO : from clap config

const FORK_SPEC_PATH: &str = "../docker/data/chainspec.dev.json";
const WRITE_TO_PATH: &str = "../docker/data/chainspec.fork.json";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data = read_file(FORK_SPEC_PATH);
    let mut fork_spec: Value = serde_json::from_str(&data)?;

    // get current chain state (storage)
    let storage: Value = reqwest::Client::new()
        // TODO : from clap config
        .post("http://127.0.0.1:9933")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "state_getPairs",
            "params": ["0x"]
        }))
        .send()
        .await?
        .json()
        .await?;

    let storage = storage["result"].as_array().unwrap();

    // move the desired storage values from the snapshot of the chain to the forked chain genesis spec
    let prefixes = ["Aura", "Aleph", /*"Session",*/ "Treasury", "Vesting"];
    storage
        .iter()
        .filter(|pair| {
            prefixes.iter().any(|prefix| {
                let pair = pair.as_array().unwrap();
                let storage_key = pair[0].as_str().unwrap();
                storage_key.starts_with(&format!("0x{}", prefix_as_hex(prefix)))
                    || storage_key.eq("0x3a636f6465")
            })
        })
        .for_each(|pair| {
            let pair = pair.as_array().unwrap();
            let k = &pair[0].as_str().unwrap();
            let v = &pair[1];
            fork_spec["genesis"]["raw"]["top"][k] = v.to_owned();
        });

    // TODO : write out the fork spec

    Ok(())
}
