use core::cmp::min;
use std::time::Duration;

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
use log::info;
use subxt::{
    ext::sp_core::{sr25519, Pair},
    tx::TxPayload,
    utils::{MultiAddress, Static},
};
use tokio::{time, time::sleep};

mod config;

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
    let n_connections = connections.len() as u64;
    let handles: Vec<_> = connections
        .into_iter()
        .enumerate()
        .map(|(conn_id, conn)| {
            let dest = dest.clone();
            tokio::spawn(async move {
                let start = time::Instant::now();
                let mut nonce = conn.account_nonce(conn.account_id()).await.unwrap();
                for i in 0..schedule.intervals {
                    let mut ok_count = 0;

                    let pending_in_pool = conn.pending_extrinsics_len().await.unwrap();
                    if conn_id == 0 {
                        log::debug!("Pool size: {pending_in_pool}");
                    }

                    let transactions_to_soft_limit = pool_limit.saturating_sub(pending_in_pool);
                    let total_tx_to_submit = min(transactions_to_soft_limit, schedule.transactions_in_interval);
                    let mut tx_to_sumbit = total_tx_to_submit / n_connections;
                    if (conn_id as u64) < total_tx_to_submit % n_connections {
                        tx_to_sumbit += 1;
                    }
                    for _ in 0..tx_to_sumbit {
                        if let Err(e) = conn.send_tx_with_params(
                            transfer_keep_alive(dest.clone(), transfer_amount),
                            Default::default(),
                            Some(nonce),
                            status
                        ).await {
                            nonce = conn.account_nonce(conn.account_id()).await.unwrap();
                            log::info!("Error when submitting a transaction: {e:?}");
                            break;
                        } else {
                            nonce += 1;
                            ok_count += 1;
                        }
                    }
                    log::debug!("Account {conn_id} round {i}: submitted #{ok_count}/{tx_to_sumbit} transactions");

                    let dur = time::Instant::now().saturating_duration_since(start);
                    if dur.as_secs() >= schedule.interval_duration * schedule.intervals {
                        break;
                    }
                    let left_duration = Duration::from_secs((i + 1) * schedule.interval_duration).saturating_sub(dur);
                    sleep(left_duration).await;
                }

                if return_balance_at_the_end {
                    conn.send_tx_with_params(
                        transfer_all(dest, true),
                        Default::default(),
                        Some(nonce),
                        status
                    ).await.unwrap();
                }
            })
        })
        .collect();

    join_all(handles).await;

    Ok(())
}

async fn initialize_n_accounts<F: Fn(u32) -> String>(
    connection: &SignedConnection,
    n: u32,
    node: F,
    amount: Balance,
    skip: bool,
) -> Vec<SignedConnection> {
    log::info!(
        "Initializing accounts, estimated total fee per account: {}TZERO",
        amount as f32 / TOKEN as f32
    );
    let mut connections = vec![];
    for i in 0..n {
        let seed = i.to_string();
        let signer = KeyPair::new(raw_keypair_from_string(&("//".to_string() + &seed)));
        connections.push(SignedConnection::new(&node(i), signer).await);
    }

    if skip {
        return connections;
    }

    let nonce = connection
        .account_nonce(connection.account_id())
        .await
        .unwrap();
    for (i, conn) in connections.iter().enumerate() {
        connection
            .send_tx_with_params(
                transfer_keep_alive(conn.account_id().clone(), amount),
                Default::default(),
                Some(nonce + i as Nonce),
                TxStatus::Submitted,
            )
            .await
            .unwrap();
    }

    connection
        .send_tx_with_params(
            transfer_keep_alive(connection.account_id().clone(), 1),
            Default::default(),
            Some(nonce + connections.len() as Nonce),
            TxStatus::Finalized,
        )
        .await
        .unwrap();

    connections
}

/// Only a rough estimation, for the worst case where blocks are 75% full with normal non-operational.
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
        fee = (fee as f64 * 1.06) as Balance;
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

fn stddev(values: &[u64]) -> f64 {
    let mean = values.iter().map(|&x| x as f64).sum::<f64>() / values.len() as f64;
    let mean_of_squares =
        values.iter().map(|&x| x as f64 * x as f64).sum::<f64>() / values.len() as f64;
    (mean_of_squares - mean * mean).sqrt()
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
    assert!(accounts >= 1);

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
    .await;

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
        "Stats:\nTransactions per second: {:.2}\nTransactions per block: {:.2} (stddev = {:.2})\nBlock time: {:.2}ms (stddev = {:.2})",
        stats.transactions_per_second,
        stats.transactions_per_block,
        stats.transactions_per_block_stddev,
        stats.block_time,
        stats.block_time_stddev,
    );

    Ok(())
}
