use core::cmp::min;

use aleph_client::{
    account_from_keypair,
    pallets::{
        author::AuthorRpc, balances::BalanceUserApi, system::SystemApi, timestamp::TimestampApi,
    },
    raw_keypair_from_string,
    utility::BlocksApi,
    AccountId, Balance, KeyPair, Nonce, SignedConnection, SignedConnectionApi, TxStatus, TOKEN,
};
use clap::Parser;
use config::Config;
use futures::future::join_all;
use log::{debug, info};
use subxt::{
    ext::sp_core::{sr25519, Pair},
    tx::TxPayload,
    utils::{MultiAddress, Static},
};
use tokio::time::{sleep, Duration, Instant};

mod config;

/// Accounts used for the experiment (except for the main account) will have seed //1 etc.
const ACCOUNTS_SEED_PREFIX: &str = "//";

fn transfer_keep_alive(dest: AccountId, amount: Balance) -> impl TxPayload + Send + Sync {
    aleph_client::api::tx()
        .balances()
        .transfer_keep_alive(MultiAddress::Id(Static(dest)), amount)
}

fn transfer_all(dest: AccountId, keep_alive: bool) -> impl TxPayload + Send + Sync {
    aleph_client::api::tx()
        .balances()
        .transfer_all(MultiAddress::Id(Static(dest)), keep_alive)
}

struct Schedule {
    pub intervals: u64,
    pub interval_duration: u64,
    pub transactions_in_interval: u64,
}

async fn flood(
    connections: Vec<SignedConnection>,
    dest: AccountId,
    transfer_amount: Balance,
    schedule: Schedule,
    status: TxStatus,
    pool_limit: u64,
    return_balance_at_the_end: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let mut nonces: Vec<u32> = vec![];
    for conn in &connections {
        nonces.push(conn.account_nonce(conn.account_id()).await?);
    }

    let mut overdue_transactions = 0;
    for i in 0..schedule.intervals {
        let pending_in_pool = connections[0].pending_extrinsics_len().await?;
        overdue_transactions += schedule.transactions_in_interval;
        let transactions_to_pool_limit = pool_limit.saturating_sub(pending_in_pool);
        let transactions_to_send = min(transactions_to_pool_limit, overdue_transactions);
        overdue_transactions -= transactions_to_send;
        info!("Interval {}: submitting {} txns", i, transactions_to_send);
        debug!("In txpool there are {pending_in_pool} txns. Overdue txns: {overdue_transactions}");

        let tasks = connections
            .iter()
            .enumerate()
            .map(|(conn_id, conn)| {
                let mut per_task = transactions_to_send / connections.len() as u64;
                if (conn_id as u64) < transactions_to_send % connections.len() as u64 {
                    per_task += 1;
                }

                let dest = dest.clone();
                let nonce = nonces[conn_id];
                nonces[conn_id] += per_task as u32;
                async move {
                    for tx_id in 0..per_task as u32 {
                        conn.sign_with_params(
                            transfer_keep_alive(dest.clone(), transfer_amount),
                            Default::default(),
                            nonce + tx_id,
                        )?
                        .submit(status)
                        .await
                        .unwrap();
                    }
                    anyhow::Ok(())
                }
            })
            .collect::<Vec<_>>();

        join_all(tasks).await;

        let duration = Instant::now().saturating_duration_since(start);
        if duration.as_secs() >= schedule.interval_duration * schedule.intervals {
            break;
        }
        let left_duration =
            Duration::from_secs((i + 1) * schedule.interval_duration).saturating_sub(duration);
        sleep(left_duration).await;
    }

    info!("Flooding time has passed, left {overdue_transactions} overdue txns because of the pool limit.");
    if return_balance_at_the_end {
        for (conn_id, conn) in connections.iter().enumerate() {
            conn.sign_with_params(
                transfer_all(dest.clone(), true),
                Default::default(),
                nonces[conn_id],
            )?
            .submit(status)
            .await?;
        }
        debug!("Returned balance back to main account");
    }

    Ok(())
}

async fn initialize_n_accounts<F: Fn(u32) -> String>(
    connection: &SignedConnection,
    n: u32,
    node: F,
    amount: Balance,
    skip: bool,
) -> anyhow::Result<Vec<SignedConnection>> {
    info!(
        "Initializing accounts, estimated total fee per account: {}TZERO",
        amount as f32 / TOKEN as f32
    );
    let mut connections = vec![];
    for i in 0..n {
        let seed = i.to_string();
        let signer = KeyPair::new(raw_keypair_from_string(
            &(ACCOUNTS_SEED_PREFIX.to_string() + &seed),
        ));
        connections.push(SignedConnection::new(&node(i), signer).await);
    }

    if skip {
        return Ok(connections);
    }

    let nonce = connection.account_nonce(connection.account_id()).await?;
    for (i, conn) in connections.iter().enumerate() {
        connection
            .sign_with_params(
                transfer_keep_alive(conn.account_id().clone(), amount),
                Default::default(),
                nonce + i as Nonce,
            )?
            .submit(TxStatus::Submitted)
            .await?;
    }

    connection
        .sign_with_params(
            transfer_keep_alive(connection.account_id().clone(), 1),
            Default::default(),
            nonce + connections.len() as Nonce,
        )?
        .submit(TxStatus::Finalized)
        .await?;

    Ok(connections)
}

/// Only a rough estimation, for the worst case where blocks are 75% full
/// (it is a maximum for non-operational transactions).
async fn estimate_avg_fee_per_transaction_in_block(
    main_connection: &SignedConnection,
    schedule: &Schedule,
) -> anyhow::Result<u128> {
    let estimated_blocks = (schedule.intervals * schedule.interval_duration) as u128;
    let fee_estimation_tx = main_connection
        .transfer_keep_alive(main_connection.account_id().clone(), 1, TxStatus::Finalized)
        .await?;
    let starting_fee = main_connection.get_tx_fee(fee_estimation_tx).await?;

    let mut total_fee = 0;
    let mut fee = starting_fee;
    for _ in 0..estimated_blocks {
        total_fee += fee;
        fee = (fee as f64 * 1.065) as Balance;
        if total_fee > Balance::MAX / 4 {
            return Err(anyhow::anyhow!("Fee estimation overflowed."));
        }
    }
    Ok((total_fee + estimated_blocks - 1) / estimated_blocks)
}

struct FloodStats {
    transactions_per_second: f64,
    transactions_per_block: f64,
    transactions_per_block_stddev: f64,
    block_time: f64,
    block_time_stddev: f64,
}

async fn compute_stats(
    connection: &SignedConnection,
    start_block: u32,
    end_block: u32,
) -> anyhow::Result<FloodStats> {
    let mut xt_counts = vec![];
    let mut block_times = vec![];

    let timestamp = |number| async move {
        anyhow::Ok(
            connection
                .get_timestamp(connection.get_block_hash(number).await?)
                .await
                .unwrap(),
        )
    };

    for number in start_block..=end_block {
        let hash = connection.get_block_hash(number).await?.unwrap();
        let block = connection.connection.as_client().blocks().at(hash).await?;
        xt_counts.push(block.body().await?.extrinsics().len().try_into()?);
        block_times.push(timestamp(number).await? - timestamp(number - 1).await?);
    }

    let total_time_ms = timestamp(end_block).await? - timestamp(start_block - 1).await?;
    let total_xt: u64 = xt_counts.iter().sum();

    Ok(FloodStats {
        transactions_per_second: total_xt as f64 * 1000.0 / total_time_ms as f64,
        transactions_per_block: total_xt as f64 / xt_counts.len() as f64,
        transactions_per_block_stddev: stddev(&xt_counts[..]),
        block_time: total_time_ms as f64 / xt_counts.len() as f64,
        block_time_stddev: stddev(&block_times[..]),
    })
}

fn stddev(values: &[u64]) -> f64 {
    let mean = values.iter().map(|&x| x as f64).sum::<f64>() / values.len() as f64;
    let mean_of_squares =
        values.iter().map(|&x| x as f64 * x as f64).sum::<f64>() / values.len() as f64;
    (mean_of_squares - mean * mean).sqrt()
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let config: Config = Config::parse();
    info!("Starting benchmark with config {:#?}", &config);

    // We want to fail fast in case seed or phrase are incorrect
    if !config.skip_initialization && config.phrase.is_none() && config.seed.is_none() {
        panic!("Needs --phrase or --seed");
    }

    let schedule = Schedule {
        intervals: config.intervals,
        interval_duration: config.interval_duration,
        transactions_in_interval: config.transactions_in_interval,
    };

    let accounts: u32 = (schedule.transactions_in_interval as f64).sqrt() as u32;

    let tx_status = match config.wait_for_ready {
        true => TxStatus::InBlock,
        false => TxStatus::Submitted,
    };
    let account = match &config.phrase {
        Some(phrase) => {
            sr25519::Pair::from_phrase(&config::read_phrase(phrase.clone()), None)
                .unwrap()
                .0
        }
        None => sr25519::Pair::from_string(
            config.seed.as_ref().expect("We checked it is not None."),
            None,
        )
        .unwrap(),
    };

    let main_connection =
        SignedConnection::new(&config.nodes[0], KeyPair::new(account.clone())).await;

    let mut avg_fee_per_transaction =
        estimate_avg_fee_per_transaction_in_block(&main_connection, &schedule).await?;
    avg_fee_per_transaction = avg_fee_per_transaction * 5 / 4; // Leave some margin

    let total_fee_per_account = (avg_fee_per_transaction / accounts as u128)
        .saturating_mul(schedule.transactions_in_interval as u128)
        .saturating_mul(schedule.intervals as u128);

    let nodes = config.nodes.clone();
    let connections = initialize_n_accounts(
        &main_connection,
        accounts,
        |i| nodes[i as usize % nodes.len()].clone(),
        total_fee_per_account,
        config.skip_initialization,
    )
    .await?;

    let best_block_pre_flood = main_connection.get_best_block().await.unwrap().unwrap();

    flood(
        connections,
        account_from_keypair(&account),
        1,
        schedule,
        tx_status,
        config.pool_limit,
        !config.skip_initialization,
    )
    .await?;

    let end_block = main_connection.get_best_block().await.unwrap().unwrap();
    let start_block = best_block_pre_flood + (end_block - best_block_pre_flood) / 10;
    let stats = compute_stats(&main_connection, start_block, end_block).await?;

    info!(
        "Stats:\nActual transactions per second: {:.2}\nTransactions per block: {:.2} (stddev = {:.2})\nBlock time: {:.2}ms (stddev = {:.2})",
        stats.transactions_per_second,
        stats.transactions_per_block,
        stats.transactions_per_block_stddev,
        stats.block_time,
        stats.block_time_stddev,
    );

    Ok(())
}
