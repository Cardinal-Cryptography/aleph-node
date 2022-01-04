// build raw genesis spec
//

// dump state
// curl http://localhost:9933 -H "Content-type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"state_getPairs","params":["0x"]}' > /tmp/storage.json

use common::{create_connection, prefix_as_hex, read_file, storage_key, storage_key_hash};
use reqwest;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::format;
use std::process::Command;

const WASM_PATH: &str = "../target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm";
const CHAIN_SPEC_PATH: &str = "../docker/data/chainspec.json";
const FORK_SPEC_PATH: &str = "../docker/data/chainspec.dev.json";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO : read spec
    let data = read_file(CHAIN_SPEC_PATH);
    let mut chain_spec: Value = serde_json::from_str(&data)?;

    // println!("{:#?}", chain_spec);

    // dump the current WASM runtime code
    // cargo build -p aleph-runtime
    // cat target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm | hexdump -ve '/1 "%02x"' > /tmp/aleph.hex

    let runtime_code = Command::new("sh")
        .arg("-c")
        .arg(format!("cat {} | hexdump -ve \'/1 \"%02x\"\'", WASM_PATH))
        .output()
        .expect("failed to execute process");

    // get current chain state (storage)
    let storage: Value = reqwest::Client::new()
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

    // set "code" to hex dump of code

    // set sudo key

    // set keys from storage in new spec (see which ones)
    // session
    let prefixes = vec!["Aura", "Sudo", "Aleph", "Session", "Treasury", "Vesting"];

    println!(
        "{:#?}",
        storage_key_hash(storage_key("Aura", "Authorities"))
    );

    println!("{:#?}", prefix_as_hex("Session"));

    // println!("{:#?}", chain_spec["genesis"]["raw"]["top"]);

    let res = chain_spec["genesis"]["raw"]["top"]
        .as_object()
        .iter_mut()
        .map(|pair| {
            println!("element {:#?}", pair);
        })
        // .collect()
        ;

    Ok(())
}
