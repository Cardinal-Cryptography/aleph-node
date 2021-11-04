mod config;

use clap::Parser;
use common::{create_connection, get_env_var};
use config::Config;
use log::info;
use sp_core::crypto::Ss58Codec;
use sp_core::{sr25519, Pair};
use sp_runtime::{generic, traits::BlakeTwo256};
use std::env;
use std::sync::mpsc::channel;
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_call, compose_extrinsic, AccountId, Api, UncheckedExtrinsicV4, XtStatus,
};

type BlockNumber = u32;
type Header = generic::Header<BlockNumber, BlakeTwo256>;

fn main() -> anyhow::Result<()> {
    env::set_var(
        "RUST_LOG",
        &get_env_var("RUST_LOG", Some(String::from("warn"))),
    );
    env_logger::init();

    let config: Config = Config::parse();

    test_finalization(config.clone())?;
    test_token_transfer(config.clone())?;
    test_change_validators(config)?;

    Ok(())
}

/// wait untill blocks are getting finalized
fn test_finalization(config: Config) -> anyhow::Result<u32> {
    let connection = create_connection(format!("ws://{}", &config.node));
    // NOTE : we wait here for a whole genesis session to pass
    // session period is set to 5 blocks (see `run_consensus.sh`), plus one to be on the safe site
    wait_for_finalized_block(connection, 6)
}

fn test_token_transfer(config: Config) -> anyhow::Result<()> {
    let Config { node, seeds, .. } = config;

    let accounts: Vec<sr25519::Pair> = match seeds {
        Some(seeds) => seeds
            .into_iter()
            .map(|seed| {
                sr25519::Pair::from_string(&seed, None).expect("Can't create pair from seed value")
            })
            .collect(),
        None => vec!["//Damian", "//Tomasz", "//Zbyszko", "//Hansu"]
            .iter()
            .map(|seed| {
                sr25519::Pair::from_string(seed, None).expect("Can't create pair from seed value")
            })
            .collect(),
    };

    let from: sr25519::Pair = accounts.get(0).expect("No accounts passed").to_owned();

    let to = AccountId::from(
        accounts
            .get(1)
            .expect("Pass at least two accounts")
            .public(),
    );

    let connection = create_connection(format!("ws://{}", node)).set_signer(from);

    let balance_before = connection
        .get_account_data(&to)?
        .expect("Could not get account data")
        .free;

    info!("[+] Account {} balance before tx: {}", to, balance_before);

    let transfer_value = 1000u128;

    let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
        connection,
        "Balances",
        "transfer",
        GenericAddress::Id(to.clone()),
        Compact(transfer_value)
    );

    // send and watch extrinsic until InBlock
    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::InBlock)?
        .expect("Could not get tx hash");

    info!("[+] Transaction hash: {}", tx_hash);

    let balance_after = connection
        .get_account_data(&to)?
        .expect("Could not get account data")
        .free;

    info!("[+] Account {} balance after tx: {}", to, balance_after);

    assert!(
        balance_before + transfer_value == balance_after,
        "before = {}, after = {}, tx = {}",
        balance_before,
        balance_after,
        transfer_value
    );

    Ok(())
}

fn test_change_validators(config: Config) -> anyhow::Result<()> {
    let Config { node, seeds, sudo } = config;

    let accounts = accounts(seeds);

    let sudo = match sudo {
        Some(seed) => {
            sr25519::Pair::from_string(&seed, None).expect("Cant create Pair from seed value")
        }
        None => accounts.get(0).expect("whoops").to_owned(),
    };

    let connection = create_connection(format!("ws://{}", node)).set_signer(sudo);

    let validators_before: Vec<AccountId> = connection
        .get_storage_value("Session", "Validators", None)?
        .unwrap();

    info!("[+] Validators before tx: {:#?}", validators_before);

    let mut new_validators: Vec<AccountId> = accounts
        .into_iter()
        .map(|pair| pair.public().into())
        .collect();

    new_validators.push(
        AccountId::from_ss58check("5EHkv1FCd4jeQmVrbYhrETL1EAr8NJxNbukDRT4FaYWbjW8f").unwrap(),
    );

    info!("[+] New validators {:#?}", new_validators);

    // what session is this?

    let session_number: u32 = connection
        .get_storage_value("Session", "CurrentIndex", None)
        .unwrap()
        .unwrap();

    info!("[+] Session number {:?}", session_number);

    let call = compose_call!(
        connection.metadata,
        "Aleph",
        "change_validators",
        new_validators.clone(),
        session_number + 2
    );

    let tx = compose_extrinsic!(connection, "Sudo", "sudo_unchecked_weight", call, 0_u64);

    // send and watch extrinsic until finalized
    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::InBlock)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");

    info!("[+] Transaction hash: {}", tx_hash);

    // wait for the event

    let block_hash = connection.get_finalized_head()?.unwrap();
    let header: Header = connection.get_header(Some(block_hash))?.unwrap();
    let block_number = header.number;

    info!("[+] Current block number: {}", block_number);

    // NOTE: this is hackish, we are assuming that two blocks is enough
    // ideally we should wait until the event arrives
    // see `api::subscribe_events`
    // but I could not readily get it to work with this event
    let _current_block_number = wait_for_finalized_block(connection.clone(), header.number + 2)?;

    let validators_after: Vec<AccountId> = connection
        .get_storage_value("Session", "Validators", None)?
        .unwrap();

    info!("[+] Validators after tx: {:#?}", validators_after);

    assert!(new_validators.eq(&validators_after));

    Ok(())
}

/// blocks the main thread waiting for a block with a number at least `block_number`
fn wait_for_finalized_block(
    connection: Api<sr25519::Pair, WsRpcClient>,
    block_number: u32,
) -> anyhow::Result<u32> {
    let (sender, receiver) = channel();
    connection.subscribe_finalized_heads(sender)?;

    while let Ok(header) = receiver
        .recv()
        .map(|h| serde_json::from_str::<Header>(&h).unwrap())
    {
        info!("[+] Received header for a block number {:?}", header.number);

        if header.number.ge(&block_number) {
            return Ok(block_number);
        }
    }

    Err(anyhow::anyhow!("Giving up"))
}

fn accounts(seeds: Option<Vec<String>>) -> Vec<sr25519::Pair> {
    match seeds {
        Some(seeds) => seeds
            .into_iter()
            .map(|seed| {
                sr25519::Pair::from_string(&seed, None).expect("Can't create pair from seed value")
            })
            .collect(),
        None => vec!["//Damian", "//Tomasz", "//Zbyszko", "//Hansu"]
            .iter()
            .map(|seed| {
                sr25519::Pair::from_string(seed, None).expect("Can't create pair from seed value")
            })
            .collect(),
    }
}
