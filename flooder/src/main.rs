use aleph_client::TOKEN;
use core::cmp::min;
use std::env;
use std::time::{Duration, Instant};

use aleph_client::{
    account_from_keypair, pallets::balances::BalanceUserApi, raw_keypair_from_string, AccountId,
    Balance, BlockHash, Connection, ConnectionApi, KeyPair, SignedConnection, SignedConnectionApi,
    TxStatus,
};

use subxt::rpc::{Rpc, RpcParams};
use subxt::tx::TxClient;
use subxt::utils::MultiAddress;

use aleph_client::AsConnection;
use clap::Parser;
use config::Config;
use futures::future::join_all;
use log::info;
use sp_core::Bytes;
use subxt::config::extrinsic_params::BaseExtrinsicParamsBuilder;
use subxt::ext::sp_core::{sr25519, Pair};
use subxt::rpc_params;
use subxt::tx::PairSigner;
use tokio::{time, time::sleep};

mod config;

pub async fn get_nonce(connection: &Connection, account: &AccountId) -> u64 {
    connection.client.tx().account_nonce(account).await.unwrap()
}

pub async fn pending_extrinsics_in_pool(connection: &Connection) -> anyhow::Result<usize> {
    Ok(connection
        .client
        .rpc()
        .request::<Vec<Bytes>>("author_pendingExtrinsics", rpc_params![])
        .await?
        .len())
}

pub async fn transfer_keep_alive(
    connection: &SignedConnection,
    dest: &AccountId,
    transfer_amount: Balance,
    nonce: u64,
    status: TxStatus,
) -> Result<(), String> {
    let tx = aleph_client::api::tx()
        .balances()
        .transfer_keep_alive(MultiAddress::Id(dest.clone().into()), transfer_amount);

    let watcher = connection
        .connection
        .client
        .tx()
        .create_signed_with_nonce(
            &tx,
            &PairSigner::new(connection.signer.signer().clone()),
            nonce,
            BaseExtrinsicParamsBuilder::new(),
        )
        .unwrap()
        .submit_and_watch()
        .await
        .map_err(|e| e.to_string())?;

    if let TxStatus::InBlock = status {
        watcher
            .wait_for_in_block()
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// Max number of ready transactions a node can store (see --pool-limit flag in aleph-node).
const TX_POOL_LIMIT: u64 = 8096;
// Leave some space for non-flooder transactions.
const TX_POOL_LIMIT_SOFT: u64 = TX_POOL_LIMIT * 3 / 4;

async fn flood(
    connections: Vec<SignedConnection>,
    dest: AccountId,
    transfer_amount: Balance,
    transactions_in_interval: u64,
    interval_secs: u64,
    duration: u64,
    status: TxStatus,
) -> anyhow::Result<()> {
    let n_connections = connections.len() as u64;
    let handles: Vec<_> = connections
        .into_iter()
        .enumerate()
        .map(|(conn_id, conn)| {
            let dest = dest.clone();
            tokio::spawn(async move {
                let start = time::Instant::now();
                let mut nonce = get_nonce(&conn.as_connection(), conn.account_id()).await;
                for i in 0..duration {
                    let mut ok_count = 0;

                    let pending_in_pool: u64 = pending_extrinsics_in_pool(&conn.as_connection())
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap();
                    if conn_id == 0 {
                        log::debug!("Pool size: {pending_in_pool}");
                    }

                    let transactions_to_soft_limit = TX_POOL_LIMIT_SOFT.saturating_sub(pending_in_pool);
                    let total_tx_to_submit = min(transactions_to_soft_limit, transactions_in_interval);
                    let mut tx_to_sumbit = total_tx_to_submit / n_connections as u64;
                    if (conn_id as u64) < total_tx_to_submit % n_connections {
                        tx_to_sumbit += 1;
                    }
                    for _ in 0..tx_to_sumbit {
                        if let Err(e) = transfer_keep_alive(
                            &conn,
                            &dest,
                            transfer_amount,
                            nonce,
                            TxStatus::Submitted,
                        )
                        .await
                        {
                            nonce = get_nonce(&conn.as_connection(), conn.account_id()).await;
                            log::info!("Error when submitting a transaction: {e:?}");
                            break;
                        } else {
                            nonce += 1;
                            ok_count += 1;
                        }
                    }
                    log::debug!("Account {conn_id} round {i}: submitted #{ok_count}/{tx_to_sumbit} transactions");

                    let dur = time::Instant::now().saturating_duration_since(start);
                    if dur.as_secs() >= duration * interval_secs {
                        break;
                    }
                    let left_duration = Duration::from_secs((i+1)*interval_secs).saturating_sub(dur);
                    sleep(left_duration).await;
                }
            })
        })
        .collect();

    join_all(handles).await;

    Ok(())
}

async fn initialize_n_accounts<F: Fn(u32) -> String>(
    connection: SignedConnection,
    n: u32,
    node: F,
    skip: bool,
) -> Vec<SignedConnection> {
    let mut connections = vec![];
    for i in 0..n {
        let seed = i.to_string();
        let signer = KeyPair::new(raw_keypair_from_string(&("//".to_string() + &seed)));
        connections.push(SignedConnection::new(&node(i), signer).await);
    }

    if skip {
        return connections;
    }

    let nonce = get_nonce(connection.as_connection(), connection.account_id()).await;
    for (i, conn) in connections.iter().enumerate() {
        transfer_keep_alive(
            &connection,
            &conn.account_id(),
            (1e22 as u128).into(),
            nonce + i as u64,
            if i % 100 == 0 {
                TxStatus::InBlock
            } else {
                TxStatus::Submitted
            },
        )
        .await;
    }
    transfer_keep_alive(
        &connection,
        &connection.account_id(),
        1,
        nonce + connections.len() as u64,
        TxStatus::Finalized,
    )
    .await;

    connections
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    let config: Config = Config::parse();
    info!("Starting benchmark with config {:#?}", &config);

    // we want to fail fast in case seed or phrase are incorrect
    if !config.skip_initialization && config.phrase.is_none() && config.seed.is_none() {
        panic!("Needs --phrase or --seed");
    }

    let accounts: u32 = min(32u64, config.transactions_in_interval)
        .try_into()
        .unwrap();

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

    let nodes = config.nodes.clone();
    let connections = initialize_n_accounts(
        main_connection,
        accounts,
        |i| nodes[i as usize % nodes.len()].clone(),
        config.skip_initialization,
    )
    .await;

    flood(
        connections,
        account_from_keypair(&account),
        1,
        config.transactions_in_interval,
        config.interval_secs,
        config.duration,
        tx_status,
    )
    .await?;

    Ok(())
}
