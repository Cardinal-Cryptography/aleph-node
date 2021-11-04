use log::warn;
use sp_core::sr25519;
use std::env;
use std::thread::sleep;
use std::time::Duration;
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::Api;

pub fn create_connection(url: String) -> Api<sr25519::Pair, WsRpcClient> {
    let client = WsRpcClient::new(&url);
    match Api::<sr25519::Pair, _>::new(client) {
        Ok(api) => api,
        Err(why) => {
            warn!(
                "[+] Can't create_connection becasue {:?}, will try again in 1s",
                why
            );
            sleep(Duration::from_millis(1000));
            create_connection(url)
        }
    }
}

pub fn get_env_var(var: &str, default: Option<String>) -> String {
    match env::var(var) {
        Ok(v) => v,
        Err(_) => match default {
            None => panic!("Missing ENV variable: {} not defined in environment", var),
            Some(d) => d,
        },
    }
}
