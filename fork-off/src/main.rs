// build raw genesis spec
//

// dump state
// curl http://localhost:9933 -H "Content-type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"state_getPairs","params":["0x"]}' > /tmp/storage.json

use common::create_connection;
use reqwest;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::format;
use std::process::Command;

// const WASM_PATH: &str = "/home/filip/CloudStation/aleph/aleph-node/target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm";
const WASM_PATH: &str = "../target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    // dump state
    // let body: serde_json::Value = client
    //     .post("http://127.0.0.1:9933")
    //     .json(&serde_json::json!({
    //         "jsonrpc": "2.0",
    //         "id": 1,
    //         "method": "state_getPairs",
    //         "params": ["0x"]
    //     }))
    //     .send()
    //     .await?
    //     .json()
    //     .await?;

    let body = None::<u32>
        // client
        // .post("http://127.0.0.1:9933")
        // .json(&serde_json::json!({
        //     "jsonrpc": "2.0",
        //     "id": 1,
        //     "method": "state_getPairs",
        //     "params": ["0x"]
        // }))
        // .send()
        // .await?
        // .text()
        // .await?
        ;

    // TODO : dump it to a file
    println!("{:#?}", body);

    // dump the current WASM code
    // cargo build -p aleph-runtime
    // cat target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm | hexdump -ve '/1 "%02x"' > /tmp/aleph.hex

    let a = Command::new("sh")
        .arg("-c")
        .arg(format!("cat {} | hexdump -ve \'/1 \"%02x\"\'", WASM_PATH))
        .output()
        .expect("failed to execute process");

    println!("{:#?}", a);

    Ok(())
}
