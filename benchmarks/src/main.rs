mod config;

use clap::Parser;
use codec::{Compact, Encode};
use config::Config;
use futures::future::join_all;
use hdrhistogram::Histogram as HdrHistogram;
use log::{debug, info, warn};
use sp_core::{sr25519, DeriveJunction, Pair};
use sp_runtime::MultiAddress;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_call, compose_extrinsic_offline, AccountId, Api, GenericAddress, UncheckedExtrinsicV4,
    XtStatus,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let config: Config = Config::parse();
    info!("Starting benchmark with config {:#?}", &config);

    let (account, _seed) =
        sr25519::Pair::from_phrase(&config::read_phrase(config.phrase), None).unwrap();

    let pool = create_connection_pool(config.nodes);
    let connection = pool.get(0).unwrap();

    let total_users = config.throughput;

    let existential_deposit = connection.get_existential_deposit().unwrap();
    let transfer_amount = 1u128;
    let tx_fee = 1000_000_000; /*73_268_000u128;*/
    let total_amount = existential_deposit + tx_fee + transfer_amount;

    debug!(
        "Using account {} to derive and fund accounts",
        &account.public()
    );

    assert!(
        get_funds(connection.clone(), &AccountId::from(account.public()))
            .ge(&(total_amount * total_users as u128)),
        "Account is too poor"
    );

    let mut users = Vec::with_capacity(total_users as usize);
    let mut nonces = Vec::with_capacity(total_users as usize);
    let mut account_nonce = get_nonce(connection.clone(), &AccountId::from(account.public()));

    for index in 0..total_users {
        let path = Some(DeriveJunction::soft(index));
        let (derived, _seed) = account.clone().derive(path.into_iter(), None).unwrap();

        // transfer existential deposit + tx fees (upper bound) + transfer_amount
        let tx = sign_tx(
            connection.clone(),
            account.clone(),
            account_nonce,
            AccountId::from(derived.public()),
            total_amount,
        )
        .await;

        let hash = if index.eq(&(total_users - 1)) {
            connection
                .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
                .expect("Could not send transaction")
        } else {
            connection
                .send_extrinsic(tx.hex_encode(), XtStatus::Ready)
                .expect("Could not send transaction")
        };

        account_nonce += 1;

        let nonce = get_nonce(connection.clone(), &AccountId::from(derived.public()));

        println!(
            "account {} with nonce {} will receive funds, tx hash {:?}",
            &derived.public(),
            nonce,
            hash
        );

        nonces.push(nonce);
        users.push(derived);
    }

    // sleep some to wait for the last tx hash to be finalized
    // async { sleep(Duration::from_millis(2000)) }.await;

    debug!("all accounts have received funds");

    let total_threads = config.threads;
    let total_batches = config.transactions / config.throughput;
    let users_per_thread = total_users / total_threads;
    let transactions_per_batch = config.throughput / config.threads;

    // prepare txs
    let mut txs = Vec::new();
    let mut tx_counter = 0;

    for thread in 0..total_threads {
        let mut txs_batches = Vec::new();
        for batch in 0..total_batches {
            let mut txs_batch = Vec::new();
            for user in (thread * users_per_thread)..(thread + 1) * users_per_thread {
                let connection = connection.clone();
                let from = users.get(user as usize).unwrap().to_owned();

                let call = compose_call!(
                    connection.metadata,
                    "Balances",
                    "transfer",
                    GenericAddress::Id(AccountId::from(account.public())),
                    Compact(transfer_amount)
                );

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
                tx_counter += 1;
                txs_batch.push(tx);

                println!(
                    "thread: {} batch: {} user: {}",
                    thread,
                    batch,
                    &from.public()
                );
            }
            txs_batches.push(txs_batch);
        }
        txs.push(txs_batches);
    }

    println!("Prepared {} signed txs", tx_counter);

    let histogram = Arc::new(Mutex::new(
        HdrHistogram::<u64>::new_with_bounds(1, u64::max_value(), 3).unwrap(),
    ));

    println!(
        "threads: {}, batches: {}, txs per batch: {}",
        &txs.len(),
        &txs[0].len(),
        &txs[0][0].len()
    );

    let tick = Instant::now();
    // send txs
    for batch in 0..total_batches {
        let mut futures_batch = Vec::new();

        println!("Prepared batch {}", batch);

        for thread in 0..total_threads {
            println!("sending from thread {}", thread);

            for tx in 0..transactions_per_batch {
                // retrieve signed txs
                let signed_tx = &txs[thread as usize][batch as usize][tx as usize];

                // send txs using a connection pool
                futures_batch.push(send_tx(
                    pool.get(tx as usize % pool.len()).unwrap().to_owned(),
                    signed_tx.to_owned(),
                    Arc::clone(&histogram),
                ));
            }
        }
        // block on batches of futures
        join_all(futures_batch).await;
    }

    let tock = tick.elapsed().as_millis();
    let histogram = histogram.lock().unwrap();

    println!("Summary:\n Transactions sent: {}\n Total time:        {} ms\n Slowest tx:        {} ms\n Fastest tx:        {} ms\n Average:           {:.1} ms\n Throughput:        {:.1} tx/s",
             histogram.len (),
             tock,
             histogram.max (),
             histogram.min (),
             histogram.mean (),
             1000.0 * histogram.len () as f64 / tock as f64
    );

    Ok(())
}

async fn sign_tx(
    connection: Api<sr25519::Pair, WsRpcClient>,
    signer: sr25519::Pair,
    nonce: u32,
    to: AccountId,
    amount: u128,
) -> UncheckedExtrinsicV4<([u8; 2], MultiAddress<AccountId, ()>, codec::Compact<u128>)> {
    let call = compose_call!(
        connection.metadata,
        "Balances",
        "transfer",
        GenericAddress::Id(to),
        Compact(amount)
    );

    compose_extrinsic_offline!(
        signer,
        call,
        nonce,
        Era::Immortal,
        connection.genesis_hash,
        connection.genesis_hash,
        connection.runtime_version.spec_version,
        connection.runtime_version.transaction_version
    )
}

async fn send_tx<Call>(
    connection: Api<sr25519::Pair, WsRpcClient>,
    tx: UncheckedExtrinsicV4<Call>,
    histogram: Arc<Mutex<HdrHistogram<u64>>>,
) where
    Call: Encode,
{
    let start_time = Instant::now();

    connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Ready)
        .expect("Could not send transaction");

    let elapsed_time = start_time.elapsed().as_millis();

    let mut hist = histogram.lock().unwrap();
    *hist += elapsed_time as u64;
}

fn create_connection_pool(nodes: Vec<String>) -> Vec<Api<sr25519::Pair, WsRpcClient>> {
    nodes
        .into_iter()
        .map(|url| create_connection(format!("ws://{}", &url)))
        .collect()
}

fn create_connection(url: String) -> Api<sr25519::Pair, WsRpcClient> {
    let client = WsRpcClient::new(&url);
    match Api::<sr25519::Pair, _>::new(client) {
        Ok(api) => api,
        Err(_) => {
            warn!("[+] Can't create_connection atm, will try again in 1s");
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
