// build raw genesis spec
//

// dump state
// curl http://localhost:9933 -H "Content-type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"state_getPairs","params":["0x"]}' > /tmp/storage.json

use common::{create_connection, prefix_as_hex, read_file, storage_key, storage_key_hash};
use reqwest;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::format;
use std::process::Command;
use std::str;

const WASM_PATH: &str = "../target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm";
const CHAIN_SPEC_PATH: &str = "../docker/data/chainspec.json";
const FORK_SPEC_PATH: &str = "../docker/data/chainspec.dev.json";
const WRITE_TO_PATH: &str = "../docker/data/chainspec.fork.json";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO : read spec
    let data = read_file(CHAIN_SPEC_PATH);
    let chain_spec: Value = serde_json::from_str(&data)?;

    let data = read_file(FORK_SPEC_PATH);
    let mut fork_spec: Value = serde_json::from_str(&data)?;

    // println!("{:#?}", chain_spec);

    // dump the current WASM runtime code
    // cargo build -p aleph-runtime
    // cat target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm | hexdump -ve '/1 "%02x"' > /tmp/aleph.hex

    let runtime_code = Command::new("sh")
        .arg("-c")
        .arg(format!("cat {} | hexdump -ve \'/1 \"%02x\"\'", WASM_PATH))
        .output()
        .expect("failed to execute process")
        .stdout;

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

    let storage = storage["result"].as_array().unwrap();

    // set "code" to the hex dump of code
    // fork_spec["genesis"]["raw"]["top"]["0x3a636f6465"] =
    //     Value::String(format!("0x{:?}", str::from_utf8(&runtime_code)));

    // TODO move the desired storage values over from the snapshot of the chain to the forked chain genesis spec

    let prefixes = ["Aura", "Aleph", "Session", "Treasury", "Vesting"];

    storage
        .iter()
        .filter(|pair| {
            // println!("@ {:?}", pair);

            prefixes.iter().any(|prefix| {
                let pair = pair.as_array().unwrap();
                let storage_key = pair[0].as_str().unwrap();

                // println!("@@ {:?} {:?}", storage_key, prefix_as_hex(prefix));

                storage_key.starts_with(&format!("0x{}", prefix_as_hex(prefix)))
            })
        })
        .for_each(|pair| {
            println!("@@@ {:?}", pair);
        });

    // storage
    //    .filter((i) => prefixes.some((prefix) => i[0].startsWith(prefix)))
    //    .forEach(([key, value]) => (forkedSpec.genesis.raw.top[key] = value));

    // println!("{}", fork_spec["genesis"]["raw"]["top"]["0x3a636f6465"]);

    // TODO set keys from storage in new spec (see which ones)

    // let mut genesis_block = fork_spec["genesis"]["raw"]["top"].as_object_mut().unwrap();
    // for (k, v) in chain_spec["genesis"]["raw"]["top"].as_object().unwrap() {
    //     // println!("{:#?}", k);

    //     let k = k.as_str();
    //     if k.eq("0x3a636f6465") {
    //         genesis_block.insert(k.to_owned(), Value::String("0x0".to_string()));
    //     } else if k.eq(&prefix_as_hex("Aura")) {
    //         //
    //     } else if k.eq(&prefix_as_hex("Session")) {
    //         //
    //     } else if k.eq(&prefix_as_hex("Session")) {
    //         //
    //     } else if k.eq(&prefix_as_hex("Session")) {
    //         //
    //     } else if k.eq(&prefix_as_hex("Session")) {
    //         //
    //     }
    // }

    // fork_spec["genesis"]["raw"]["top"] = serde_json::to_value(&genesis_block).unwrap();

    // println!(
    //     "{:#?}",
    //     storage_key_hash(storage_key("Aura", "Authorities"))
    // );

    Ok(())
}
