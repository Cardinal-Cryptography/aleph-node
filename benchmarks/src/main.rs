mod config;

use clap::Parser;
use codec::{Compact, Encode};
use config::Config;
use futures::future::join_all;
use futures::Future;
use hdrhistogram::Histogram as HdrHistogram;
use log::{debug, info};
use ndarray::Array3;
use sp_core::{sr25519, DeriveJunction, Pair, H256};
// use std::fmt::Result;
use std::os::unix::thread;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::thread::sleep;
use std::time::{Duration, Instant};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_call, compose_extrinsic, compose_extrinsic_offline, AccountId, Api, GenericAddress,
    UncheckedExtrinsicV4, XtStatus,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let config: Config = Config::parse();
    info!("Starting benchmark with config {:#?}", &config);

    let (account, _seed) =
        sr25519::Pair::from_phrase(&config::read_phrase(config.phrase), None).unwrap();

    let connection = create_connection(format!("ws://{}", &config.node_url));
    let transfer_value = connection.get_existential_deposit().unwrap();
    let total_users = config.througput;

    debug!(
        "Using account {} to derive and fund accounts",
        &account.public()
    );

    assert!(
        get_funds(connection.clone(), &AccountId::from(account.public()))
            .ge(&(transfer_value * total_users as u128)),
        "Too poor"
    );

    let mut users = Vec::with_capacity(total_users as usize);
    let mut nonces = Vec::with_capacity(total_users as usize);

    for index in 0..total_users {
        let path = Some(DeriveJunction::soft(index));
        let (derived, _seed) = account.clone().derive(path.into_iter(), None).unwrap();

        // let call = compose_call!(
        //     connection.metadata,
        //     "Balances",
        //     "transfer",
        //     GenericAddress::Id(AccountId::from(derived.public())),
        //     Compact(transfer_value)
        // );

        // let nonce = get_nonce(connection.clone(), &AccountId::from(account.public()));

        // let tx: UncheckedExtrinsicV4<_> = compose_extrinsic_offline!(
        //     account,
        //     call,
        //     nonce,
        //     Era::Immortal,
        //     connection.genesis_hash,
        //     connection.genesis_hash,
        //     connection.runtime_version.spec_version,
        //     connection.runtime_version.transaction_version
        // );

        let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
            connection.clone().set_signer(account.clone()),
            "Balances",
            "transfer",
            GenericAddress::Id(AccountId::from(derived.public())),
            Compact(transfer_value)
        );

        // send and watch transfer tx until finalized
        let tx_hash = connection
            .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
            .unwrap()
            .unwrap();

        let funds = get_funds(connection.clone(), &AccountId::from(derived.public()));
        let nonce = get_nonce(connection.clone(), &AccountId::from(derived.public()));

        println!(
            "account {} with nonce {} received funds in tx {}. Account free funds {}",
            &derived.public(),
            nonce,
            tx_hash,
            funds
        );

        nonces.push(nonce);
        users.push(derived);
    }

    debug!("all accounts have received funds");

    // let mut nonces = Vec::with_capacity(total_users as usize);

    let total_threads = config.threads;
    let total_batches = config.transactions / config.througput;
    let users_per_thread = total_users / total_threads;
    let transaction_per_batch = config.througput / config.threads;

    // TODO : prepare txs
    // let mut txs = Array3::<UncheckedExtrinsicV4<_>>::default((
    //     total_threads as usize,
    //     total_batches as usize,
    //     total_users as usize,
    // ));

    // let to = AccountId::from(account.public());

    for thread in 0..total_threads {
        for batch in 0..total_batches {
            for user in (thread * users_per_thread)..(thread + 1) * users_per_thread {
                let connection = connection.clone();

                let from = users.get(user as usize).unwrap().to_owned();

                let call = compose_call!(
                    connection.metadata,
                    "Balances",
                    "transfer",
                    GenericAddress::Id(AccountId::from(account.public())),
                    Compact(transfer_value)
                );

                // let u = users.get(user as usize).unwrap().to_owned();

                let tx: UncheckedExtrinsicV4<_> = compose_extrinsic_offline!(
                    from,
                    call,
                    nonces[user as usize],
                    Era::Immortal,
                    connection.genesis_hash,
                    connection.genesis_hash,
                    connection.runtime_version.spec_version,
                    connection.runtime_version.transaction_version
                );

                nonces[user as usize] += 1;

                println!("tx {:?}", tx.hex_encode());

                println!("thread: {} batch: {} user: {}", thread, batch, user);
            }
        }
    }

    // for batch in 0..total_batches {
    //     let mut futures_batch = Vec::new();

    //     println!("Prepared batch {}", batch);

    //     for thread in 0..total_batches {
    //         for tx in 0..transaction_per_batch {
    // let tx = txs[thread][batch][tx];
    //             futures_batch.push(send_tx(connection.clone(), tx));
    //         }
    //     }
    //     join_all(futures_batch).await;
    // }

    Ok(())
}

async fn send_tx<Call>(
    connection: Api<sr25519::Pair, WsRpcClient>,
    tx: UncheckedExtrinsicV4<Call>,
) -> Option<H256>
where
    Call: Encode,
{
    connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Ready)
        .expect("Could not send transaction")
}

fn create_connection(url: String) -> Api<sr25519::Pair, WsRpcClient> {
    let client = WsRpcClient::new(&url);
    match Api::<sr25519::Pair, _>::new(client) {
        Ok(api) => api,
        Err(_) => {
            println!("[+] Can't create_connection atm, will try again in 1s");
            sleep(Duration::from_millis(1000));
            create_connection(url)
        }
    }
}

fn get_nonce(connection: Api<sr25519::Pair, WsRpcClient>, account: &AccountId) -> u32 {
    connection
        .get_account_info(account)
        .map(|acc_opt| acc_opt.map_or_else(|| 0, |acc| acc.nonce))
        .unwrap()
}

fn get_funds(connection: Api<sr25519::Pair, WsRpcClient>, account: &AccountId) -> u128 {
    match connection.get_account_data(&account).unwrap() {
        Some(data) => data.free,
        None => 0,
    }
}
