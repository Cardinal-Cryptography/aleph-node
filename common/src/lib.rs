mod ws_rpc_client;

use log::warn;
use sp_core::sr25519;
use std::{thread::sleep, time::Duration};
use substrate_api_client::{Api, RpcClient, StorageKey};
pub use ws_rpc_client::WsRpcClient;

pub trait FromStr {
    fn from_str(s: &str) -> Self;
}

impl FromStr for substrate_api_client::rpc::ws_client::WsRpcClient {
    fn from_str(url: &str) -> Self {
        substrate_api_client::rpc::ws_client::WsRpcClient::new(url)
    }
}

impl FromStr for WsRpcClient {
    fn from_str(url: &str) -> Self {
        WsRpcClient::new(url)
    }
}

pub fn create_connection(
    address: String,
) -> Api<sr25519::Pair, substrate_api_client::rpc::ws_client::WsRpcClient> {
    create_custom_connection(&address)
}

pub fn create_custom_connection<Client: FromStr + RpcClient>(
    address: &str,
) -> Api<sr25519::Pair, Client> {
    let client = Client::from_str(&format!("ws://{}", address));
    match Api::<sr25519::Pair, _>::new(client) {
        Ok(api) => api,
        Err(why) => {
            warn!(
                "[+] Can't create_connection because {:?}, will try again in 1s",
                why
            );
            sleep(Duration::from_millis(1000));
            create_custom_connection(address)
        }
    }
}

pub fn storage_key(module: &str, version: &str) -> [u8; 32] {
    let pallet_name = sp_io::hashing::twox_128(module.as_bytes());
    let postfix = sp_io::hashing::twox_128(version.as_bytes());
    let mut final_key = [0u8; 32];
    final_key[..16].copy_from_slice(&pallet_name);
    final_key[16..].copy_from_slice(&postfix);
    final_key
}

pub fn storage_key_hash(bytes: [u8; 32]) -> String {
    let storage_key = StorageKey(bytes.into());
    hex::encode(storage_key.0)
}
