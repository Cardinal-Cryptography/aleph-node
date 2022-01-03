// build raw genesis spec
//

// dump state
// curl http://localhost:9933 -H "Content-type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"state_getPairs","params":["0x"]}' > /tmp/storage.json

use common::create_connection;
use reqwest;
use serde_json::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let body: serde_json::Value = client
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

    println!("{:#?}", body);

    Ok(())
}
