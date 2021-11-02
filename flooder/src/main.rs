mod config;

use clap::Parser;
use codec::{Compact, Encode};
use config::Config;
use futures::future::join_all;
use hdrhistogram::Histogram as HdrHistogram;
use log::{debug, info, warn};
use sp_core::{sr25519, DeriveJunction, Pair};
use sp_runtime::{generic, traits::BlakeTwo256, MultiAddress, OpaqueExtrinsic};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_call, compose_extrinsic_offline, AccountId, Api, GenericAddress, UncheckedExtrinsicV4,
    XtStatus,
};

type TransferTransaction =
    UncheckedExtrinsicV4<([u8; 2], MultiAddress<AccountId, ()>, codec::Compact<u128>)>;
type BlockNumber = u32;
type Header = generic::Header<BlockNumber, BlakeTwo256>;
type Block = generic::Block<Header, OpaqueExtrinsic>;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let config: Config = Config::parse();
    info!("Starting benchmark with config {:#?}", &config);

    let account = match config.phrase {
        Some(phrase) => {
            sr25519::Pair::from_phrase(&config::read_phrase(phrase), None)
                .unwrap()
                .0
        }
        None => match config.seed {
            Some(seed) => sr25519::Pair::from_string(&seed, None).unwrap(),
            None => panic!("Needs --phrase or --seed"),
        },
    };

    let pool = create_connection_pool(config.nodes);
    let connection = pool.get(0).unwrap();

    let total_users = config.throughput;
    let total_threads = config.threads;
    let total_batches = config.transactions / config.throughput;
    let users_per_thread = total_users / total_threads;
    let transactions_per_batch = config.throughput / config.threads;
    let transfer_amount = 1u128;

    info!(
        "Using account {} to derive and fund accounts",
        &account.public()
    );

    let users_and_nonces /*(users, nonces) */= derive_user_accounts(
        connection.clone(),
        account.clone(),
        total_users,
        transfer_amount,
        total_batches,
    )
    .await;

    debug!("all accounts have received funds");

    let txs = sign_transactions(
        connection.clone(),
        account,
        users_and_nonces,
        transfer_amount,
        total_threads,
        total_batches,
        users_per_thread,
    )
    .await;

    println!(
        "Prepared {} signed txs",
        txs.len() * txs[0].len() * txs[0][0].len()
    );

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

    flood(
        pool,
        txs,
        total_threads,
        total_batches,
        transactions_per_batch,
        &histogram,
    )
    .await;

    let tock = tick.elapsed().as_millis();
    let histogram = histogram.lock().unwrap();

    println!("Summary:\n TransferTransactions sent: {}\n Total time:        {} ms\n Slowest tx:        {} ms\n Fastest tx:        {} ms\n Average:           {:.1} ms\n Throughput:        {:.1} tx/s",
             histogram.len (),
             tock,
             histogram.max (),
             histogram.min (),
             histogram.mean (),
             1000.0 * histogram.len () as f64 / tock as f64
    );

    Ok(())
}

async fn flood(
    pool: Vec<Api<sr25519::Pair, WsRpcClient>>,
    txs: Vec<Vec<Vec<TransferTransaction>>>,
    total_threads: u64,
    total_batches: u64,
    transactions_per_batch: u64,
    histogram: &Arc<Mutex<HdrHistogram<u64>>>,
) {
    // send txs
    for batch in 0..total_batches {
        let mut futures_batch = Vec::new();

        println!("Preparing batch {}", batch);

        for thread in 0..total_threads {
            println!("sending batch {} from thread {}", batch, thread);

            for tx in 0..transactions_per_batch {
                // retrieve signed txs
                let signed_tx = &txs[thread as usize][batch as usize][tx as usize];

                // send txs using a connection pool
                futures_batch.push(send_tx(
                    pool.get(tx as usize % pool.len()).unwrap().to_owned(),
                    signed_tx.to_owned(),
                    Arc::clone(histogram),
                ));
            }
        }
        // block on batches of futures
        join_all(futures_batch).await;
    }
}

async fn estimate_tx_fee(
    connection: Api<sr25519::Pair, WsRpcClient>,
    tx: &TransferTransaction,
) -> u128 {
    let block = connection.get_block::<Block>(None).unwrap().unwrap();
    let block_hash = block.header.hash();
    let fee = connection
        .get_fee_details(&tx.hex_encode(), Some(block_hash))
        .unwrap()
        .unwrap();

    let inclusion_fee = fee.inclusion_fee.unwrap();

    fee.tip + inclusion_fee.base_fee + inclusion_fee.len_fee + inclusion_fee.adjusted_weight_fee
}

async fn sign_tx(
    connection: Api<sr25519::Pair, WsRpcClient>,
    signer: sr25519::Pair,
    nonce: u32,
    to: AccountId,
    amount: u128,
) -> TransferTransaction {
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

/// prepares payload for floading
async fn sign_transactions(
    connection: Api<sr25519::Pair, WsRpcClient>,
    account: sr25519::Pair,
    users_and_nonces: (Vec<sr25519::Pair>, Vec<u32>),
    transfer_amount: u128,
    total_threads: u64,
    total_batches: u64,
    users_per_thread: u64,
) -> Vec<Vec<Vec<TransferTransaction>>> {
    let mut txs = Vec::new();
    let (users, initial_nonces) = users_and_nonces;
    let mut nonces = initial_nonces.clone();

    for thread in 0..total_threads {
        let mut txs_batches = Vec::new();
        for batch in 0..total_batches {
            let mut txs_batch = Vec::new();
            for user in (thread * users_per_thread)..(thread + 1) * users_per_thread {
                let connection = connection.clone();
                let from = users.get(user as usize).unwrap().to_owned();

                let tx = sign_tx(
                    connection,
                    from.clone(),
                    nonces[user as usize],
                    AccountId::from(account.public()),
                    transfer_amount,
                )
                .await;

                nonces[user as usize] += 1;
                txs_batch.push(tx);

                debug!(
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
    txs
}

/// returns a tuple of derived accounts and their nonces
async fn derive_user_accounts(
    connection: Api<sr25519::Pair, WsRpcClient>,
    account: sr25519::Pair,
    total_users: u64,
    transfer_amount: u128,
    txs_per_account: u64,
) -> (Vec<sr25519::Pair>, Vec<u32>) {
    let mut users = Vec::with_capacity(total_users as usize);
    let mut nonces = Vec::with_capacity(total_users as usize);
    let mut account_nonce = get_nonce(connection.clone(), &AccountId::from(account.public()));
    let existential_deposit = connection.get_existential_deposit().unwrap();

    // start with a heuristic tx fee
    let mut total_amount =
        existential_deposit + txs_per_account as u128 * (transfer_amount + 375_000_000);

    for index in 0..total_users {
        let path = Some(DeriveJunction::soft(index));
        let (derived, _seed) = account.clone().derive(path.into_iter(), None).unwrap();

        let tx = sign_tx(
            connection.clone(),
            account.clone(),
            account_nonce,
            AccountId::from(derived.public()),
            total_amount,
        )
        .await;

        // estimate fees
        if index.eq(&0) {
            let tx_fee = estimate_tx_fee(connection.clone(), &tx).await;
            info!("Estimated transfer tx fee {}", tx_fee);

            // adjust with estimated tx fee
            total_amount =
                existential_deposit + txs_per_account as u128 * (transfer_amount + tx_fee);

            assert!(
                get_funds(connection.clone(), &AccountId::from(account.public()))
                    .ge(&(total_amount * total_users as u128)),
                "Account is too poor"
            );
        }

        let hash = if index.eq(&(total_users - 1)) {
            // ensure all txs are finalized by waiting for the last one sent
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

        info!(
            "account {} with nonce {} will receive funds, tx hash {:?}",
            &derived.public(),
            nonce,
            hash
        );

        nonces.push(nonce);
        users.push(derived);
    }

    (users, nonces)
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
    match connection.get_account_data(account).unwrap() {
        Some(data) => data.free,
        None => 0,
    }
}
