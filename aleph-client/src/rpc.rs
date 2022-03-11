use crate::{Connection, SessionKeys};
use serde_json::{json, Value};
use substrate_api_client::ApiResult;

fn json_req(method: &str, params: Value, id: u32) -> Value {
    json!({
        "method": method,
        "params": params,
        "jsonrpc": "2.0",
        "id": id.to_string(),
    })
}

pub fn author_rotate_keys() -> Value {
    json_req("author_rotateKeys", Value::Null, 1)
}

pub fn rotate_keys(connection: &Connection) -> ApiResult<Option<SessionKeys>> {
    Ok(connection
        .get_request(author_rotate_keys())?
        .map(|keys| SessionKeys::from(keys)))
}

pub fn rotate_keys_raw(connection: &Connection) -> ApiResult<Option<String>> {
    Ok(connection
        .get_request(author_rotate_keys())?
        .map(|keys| keys.trim_matches('\"').to_string()))
}
