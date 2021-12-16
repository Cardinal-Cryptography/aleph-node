mod config;

use clap::Parser;
use codec::{Compact, Encode};
use common::create_connection;
use config::Config;
use hdrhistogram::Histogram as HdrHistogram;
use log::{debug, info};
use rayon::prelude::*;
use sp_core::{sr25519, Pair};
use sp_runtime::{generic, traits::BlakeTwo256, MultiAddress, OpaqueExtrinsic};
use std::{
    iter::{once, repeat, IntoIterator},
    sync::{Arc, Mutex},
    thread::yield_now,
    time::Instant,
};
use substrate_api_client::{
    compose_call, compose_extrinsic_offline, rpc::WsRpcClient, AccountId, Api, GenericAddress,
    UncheckedExtrinsicV4, XtStatus,
};

type TransferTransaction =
    UncheckedExtrinsicV4<([u8; 2], MultiAddress<AccountId, ()>, codec::Compact<u128>)>;
type BlockNumber = u32;
type Header = generic::Header<BlockNumber, BlakeTwo256>;
type Block = generic::Block<Header, OpaqueExtrinsic>;

fn main() -> Result<(), anyhow::Error> {
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
    let initialize_accounts_flag = !config.skip_initialization;

    let pool = create_connection_pool(config.nodes);
    let connection = pool.get(0).unwrap();

    let total_users = config.transactions;
    let first_account_in_range = config.first_account_in_range;
    let throughput = config.throughput;
    let transactions_per_batch = config.throughput / rayon::current_num_threads() as u64;
    let transfer_amount = 1u128;

    info!(
        "Using account {} to derive and fund accounts",
        &account.public()
    );

    let source_account_id = AccountId::from(account.public());
    let source_account_nonce = get_nonce(&connection, &source_account_id);

    let accounts = (first_account_in_range..first_account_in_range + total_users)
        .map(|seed| derive_user_account(&account, seed))
        .collect();

    let total_amount =
        estimate_amount(&connection, &account, source_account_nonce, transfer_amount);

    assert!(
        get_funds(&connection, &source_account_id).ge(&(total_amount * total_users as u128)),
        "Account is too poor"
    );

    if initialize_accounts_flag {
        initialize_accounts(
            &connection,
            &account,
            source_account_nonce,
            &accounts,
            total_amount,
        );
    }

    let nonces = accounts
        .iter()
        .map(|account| get_nonce(&connection, &AccountId::from(account.public())))
        .collect();

    debug!("all accounts have received funds");

    let txs = sign_transactions(
        &connection,
        account,
        (accounts, nonces),
        config.transactions,
        transfer_amount,
    );

    let histogram = Arc::new(Mutex::new(
        HdrHistogram::<u64>::new_with_bounds(1, u64::MAX, 3).unwrap(),
    ));

    let tick = Instant::now();

    flood(pool, txs, transactions_per_batch, throughput, &histogram);

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

fn flood(
    pool: Vec<Api<sr25519::Pair, WsRpcClient>>,
    txs: Vec<TransferTransaction>,
    transactions_per_batch: u64,
    txps: u64,
    histogram: &Arc<Mutex<HdrHistogram<u64>>>,
) {
    let transactions_per_batch = transactions_per_batch
        .try_into()
        .expect("transactions_per_batch within usize range");
    let txps = txps.try_into().expect("txps within usize range");
    let next = 1;
    let elapsed = Instant::now();
    txs.chunks(txps).for_each(|batch| {
        batch.par_chunks(transactions_per_batch).for_each(|batch| {
            println!("Sending a batch of {} transactions", &batch.len());
            batch.iter().enumerate().for_each(|(index, tx)| {
                send_tx(
                    pool.get(index % pool.len()).unwrap().to_owned(),
                    tx.to_owned(),
                    Arc::clone(histogram),
                )
            })
        });
        while elapsed.elapsed().as_secs() < next {
            yield_now();
        }
    });
}

fn estimate_tx_fee(connection: &Api<sr25519::Pair, WsRpcClient>, tx: &TransferTransaction) -> u128 {
    let block = connection.get_block::<Block>(None).unwrap().unwrap();
    let block_hash = block.header.hash();
    let fee = connection
        .get_fee_details(&tx.hex_encode(), Some(block_hash))
        .unwrap()
        .unwrap();

    let inclusion_fee = fee.inclusion_fee.unwrap();

    fee.tip + inclusion_fee.base_fee + inclusion_fee.len_fee + inclusion_fee.adjusted_weight_fee
}

fn sign_tx(
    connection: &Api<sr25519::Pair, WsRpcClient>,
    signer: &sr25519::Pair,
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

/// prepares payload for flooding
fn sign_transactions(
    connection: &Api<sr25519::Pair, WsRpcClient>,
    account: sr25519::Pair,
    users_and_nonces: (Vec<sr25519::Pair>, Vec<u32>),
    total_transactions: u64,
    transfer_amount: u128,
) -> Vec<TransferTransaction> {
    let total_transactions = usize::try_from(total_transactions)
        .expect("total_transactions should be in the range of usize");

    let (users, initial_nonces) = users_and_nonces;
    let mut nonces = initial_nonces;

    let mut result = Vec::with_capacity(total_transactions);
    for index in 0..total_transactions {
        // NOTE : assumes one tx per derived user account
        // but we could create less accounts and send them round robin fashion
        // (will need to seed them with more funds as well, tx_per_account times more to be exact)
        let from = users.get(index).unwrap().to_owned();

        let tx = sign_tx(
            connection,
            &from,
            nonces[index],
            AccountId::from(account.public()),
            transfer_amount,
        );

        nonces[index] += 1;
        result.push(tx);
    }
    result
}

fn estimate_amount(
    connection: &Api<sr25519::Pair, WsRpcClient>,
    account: &sr25519::Pair,
    account_nonce: u32,
    transfer_amount: u128,
) -> u128 {
    let existential_deposit = connection.get_existential_deposit().unwrap();
    // start with a heuristic tx fee
    let total_amount = existential_deposit + (transfer_amount + 375_000_000);

    let tx = sign_tx(
        connection,
        account,
        account_nonce,
        AccountId::from(account.public()),
        total_amount,
    );

    // estimate fees
    let tx_fee = estimate_tx_fee(&connection, &tx);
    info!("Estimated transfer tx fee {}", tx_fee);
    // adjust with estimated tx fee
    existential_deposit + (transfer_amount + tx_fee)
}

fn initialize_accounts<'a>(
    connection: &Api<sr25519::Pair, WsRpcClient>,
    source_account: &sr25519::Pair,
    mut source_account_nonce: u32,
    accounts: &Vec<sr25519::Pair>,
    total_amount: u128,
) {
    let status = repeat(XtStatus::Ready)
        .take(accounts.len() - 1)
        .chain(once(XtStatus::Finalized));
    for (derived, status) in accounts.iter().zip(status) {
        source_account_nonce = initialize_account(
            &connection,
            &source_account,
            source_account_nonce,
            derived,
            total_amount,
            status,
        );
    }
}

fn initialize_account(
    connection: &Api<sr25519::Pair, WsRpcClient>,
    account: &sr25519::Pair,
    account_nonce: u32,
    derived: &sr25519::Pair,
    total_amount: u128,
    status: XtStatus,
) -> u32 {
    let tx = sign_tx(
        connection,
        account,
        account_nonce,
        AccountId::from(derived.public()),
        total_amount,
    );

    let hash = Some(
        connection
            .send_extrinsic(tx.hex_encode(), status)
            .expect("Could not send transaction"),
    );
    info!(
        "account {} will receive funds, tx hash {:?}",
        &derived.public(),
        hash
    );

    account_nonce + 1
}

fn derive_user_account(_account: &sr25519::Pair, seed: u64) -> sr25519::Pair {
    sr25519::Pair::from_string(&("//".to_string() + &seed.to_string()), None).unwrap()
}

fn send_tx<Call>(
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
        .map(|url| create_connection(url))
        .collect()
}

fn get_nonce(connection: &Api<sr25519::Pair, WsRpcClient>, account: &AccountId) -> u32 {
    connection
        .get_account_info(account)
        .map(|acc_opt| acc_opt.map_or_else(|| 0, |acc| acc.nonce))
        .expect("retrieved nonce's value")
}

fn get_funds(connection: &Api<sr25519::Pair, WsRpcClient>, account: &AccountId) -> u128 {
    match connection.get_account_data(account).unwrap() {
        Some(data) => data.free,
        None => 0,
    }
}
