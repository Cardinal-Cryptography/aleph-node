use crate::{Connection, SessionKeys};
use serde_json::{json, Value};
use substrate_api_client::StorageKey;

fn json_req(method: &str, params: Value, id: u32) -> Value {
    json!({
        "method": method,
        "params": params,
        "jsonrpc": "2.0",
        "id": id.to_string(),
    })
}

pub fn author_rotate_keys_json() -> Value {
    json_req("author_rotateKeys", Value::Null, 1)
}

fn state_query_storage_at_json(storage_keys: &Vec<StorageKey>) -> Value {
    json_req(
        "state_queryStorageAt",
        Value::Array(vec![
            Value::Array(
                storage_keys
                    .into_iter()
                    .map(|storage_key| Value::String(hex::encode(storage_key)))
                    .collect::<Vec<_>>(),
            ),
            Value::Null,
        ]),
        1,
    )
}

fn parse_query_storage_at_result(
    maybe_json_result: Option<String>,
) -> Result<Vec<Option<String>>, String> {
    match maybe_json_result {
        None => Err(String::from("Returned result was null!")),
        Some(result) => {
            let mut storage_entries_in_hex_string = vec![];

            let parsed_json_value: Value = serde_json::from_str(&result[..]).map_err(|_| {
                String::from(&format!("Failed to parse result {:?} into JSON", result))
            })?;
            let storage_entries = match parsed_json_value[0]["changes"].as_array() {
                Some(storage_entries) => storage_entries,
                None => {
                    return Err(String::from(&format!(
                        "Parsed json value {:?} does not have \"changes\" element!",
                        parsed_json_value
                    )))
                }
            };
            for entry in storage_entries {
                let storage_key_and_entry = match entry.as_array() {
                    Some(entry) => entry,
                    None => {
                        return Err(String::from(&format!(
                            "Value {:?} is not an array!",
                            entry.clone()
                        )))
                    }
                };
                if storage_key_and_entry.len() != 2 {
                    return Err(String::from(&format!(
                        "Expected {:?} to be length of 2!",
                        storage_key_and_entry
                    )));
                }

                let maybe_entry_in_hex_string = storage_key_and_entry[1]
                    .as_str()
                    .map(|entry| String::from(entry));
                storage_entries_in_hex_string.push(maybe_entry_in_hex_string);
            }
            Ok(storage_entries_in_hex_string)
        }
    }
}

pub fn state_query_storage_at(
    connection: &Connection,
    storage_keys: Vec<StorageKey>,
) -> Result<Vec<Option<String>>, String> {
    match connection.get_request(state_query_storage_at_json(&storage_keys)) {
        Ok(maybe_json_result) => parse_query_storage_at_result(maybe_json_result),
        Err(_) => Err(String::from(format!(
            "Failed to obtain results from storage keys {:?}",
            &storage_keys
        ))),
    }
}

pub fn rotate_keys_base<F, R>(
    connection: &Connection,
    rpc_result_mapper: F,
) -> Result<R, &'static str>
where
    F: Fn(String) -> Option<R>,
{
    match connection.get_request(author_rotate_keys_json()) {
        Ok(maybe_keys) => match maybe_keys {
            Some(keys) => match rpc_result_mapper(keys) {
                Some(keys) => Ok(keys),
                None => Err("Failed to parse keys from string result"),
            },
            None => Err("Failed to retrieve keys from chain"),
        },
        Err(_) => Err("Connection does not work"),
    }
}

pub fn rotate_keys(connection: &Connection) -> Result<SessionKeys, &'static str> {
    rotate_keys_base(connection, |keys| match SessionKeys::try_from(keys) {
        Ok(keys) => Some(keys),
        Err(_) => None,
    })
}

pub fn rotate_keys_raw_result(connection: &Connection) -> Result<String, &'static str> {
    // we need to escape two characters from RPC result which is escaped quote
    rotate_keys_base(connection, |keys| Some(keys.trim_matches('\"').to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_some_input_when_state_query_storage_at_json_then_json_is_as_expected() {
        let mut storage_keys = Vec::new();
        storage_keys.push(StorageKey(vec![0, 1, 2, 3, 4, 5]));
        storage_keys.push(StorageKey(vec![9, 8, 7, 6, 5]));
        let expected_json_string = r#"
{
   "id": "1",
   "jsonrpc": "2.0",
   "method":"state_queryStorageAt",
   "params": [["000102030405", "0908070605"], null]
}"#;

        let expected_json: Value = serde_json::from_str(expected_json_string).unwrap();
        assert_eq!(expected_json, state_query_storage_at_json(&storage_keys));
    }

    #[test]
    fn given_expected_input_when_parse_query_storage_at_result_then_json_is_as_expected() {
        let expected_json_string = r#"
 [
    {
      "block": "0x07825c4cae90d07a322ea434ed82186091e0aae8d465274d07ab1e1dea545d0d",
      "changes": [
        [
          "0xc2261276cc9d1f8598ea4b6a74b15c2f218f26c73add634897550b4003b26bc61b614bd4a126f2d5d294e9a8af9da25248d7e931307afb4b68d8d565d4c66e00d856c6d65f5fed6bb82dcfb60e936c67",
          "0x047374616b696e672000407a10f35a0000000000000000000002"
        ],
        [
          "0xc2261276cc9d1f8598ea4b6a74b15c2f218f26c73add634897550b4003b26bc6e2c1dc507e2035edbbd8776c440d870460c57f0008067cc01c5ff9eb2e2f9b3a94299a915a91198bd1021a6c55596f57",
          "0x047374616b696e672000407a10f35a0000000000000000000002"
        ],
        [
          "0xc2261276cc9d1f8598ea4b6a74b15c2f218f26c73add634897550b4003b26bc6e2c1dc507e2035edbbd8776c440d870460c57f0008067cc01c5ff9eb2e2f9b3a94299a915a91198bd1021a6c55596f59",
          null
        ]
      ]
    }
  ]"#;
        assert_eq!(
            vec![
                Some(String::from(
                    "0x047374616b696e672000407a10f35a0000000000000000000002"
                )),
                Some(String::from(
                    "0x047374616b696e672000407a10f35a0000000000000000000002"
                )),
                None
            ],
            parse_query_storage_at_result(Some(String::from(expected_json_string))).unwrap()
        );
    }
}
