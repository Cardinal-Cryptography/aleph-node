mod config;

use crate::config::accounts;
use clap::Parser;
use config::Config;
use futures::future::join_all;
use futures::{Future, Stream};
use log::{debug, info};
use rand::Rng;
use sp_core::crypto::Ss58Codec;
use sp_core::{sr25519, Pair};
use std::cmp;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_call, compose_extrinsic, AccountId, Api, UncheckedExtrinsicV4, XtStatus,
};

// TODO : tokio runtime that spawns task / core ?
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let config: Config = Config::parse();
    info!("Starting benchmark with config {:#?}", &config);

    let mut tasks = Vec::with_capacity(config.concurrency);
    let counter = Arc::new(Mutex::new(0));

    let concurrency: u64 = config.concurrency as u64;
    let n_transactions = config.throughput * config.duration;
    let threshold = n_transactions / concurrency;

    let accounts = config::accounts(config.base_path, config.account_ids, config.key_filename);

    for id in 0..config.concurrency {
        let counter = Arc::clone(&counter);
        let accounts = accounts.clone();
        let url = format!("ws://{}:{}", config.host, config.port);

        tasks.push(tokio::spawn(async move {
            let client = Client::new(id, counter, threshold, n_transactions, url, accounts);
            client.await;
        }));
    }

    // NOTE: we measure time after all the configuration
    let tick = Instant::now();

    join_all(tasks).await;

    let tock = tick.elapsed().as_secs();
    let txs = *counter.lock().unwrap();
    let time = cmp::max(1, tock);

    info!(
        "Summary:\n Total transactions sent: {}\n Elapsed time: {} s\n Theoretical throughput {:.1} tx/s\n Real throughput: {:.1} tx/s",
        txs,
        time,
        config.throughput,
        txs / time
    );

    Ok(())
}

struct Client {
    /// client id
    id: usize,
    /// counter shared between all spawned clients
    counter: Arc<Mutex<u64>>,
    /// keep sending tx until counter reaches treshold value
    threshold: u64,
    // /// timer initiated at this clock value
    // tick: Instant,
    /// total number of txs to send
    total: u64,
    /// URL for ws connection
    url: String,
    /// accounts for signing tx and sendig them to
    accounts: Vec<sr25519::Pair>,
}

impl Client {
    fn new(
        id: usize,
        counter: Arc<Mutex<u64>>,
        threshold: u64,
        total: u64,
        url: String,
        accounts: Vec<sr25519::Pair>,
    ) -> Self {
        Self {
            id,
            counter,
            threshold,
            total,
            url,
            accounts,
        }
    }
}

impl Future for Client {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        let this = &mut *self;

        let client = WsRpcClient::new(&this.url);

        let index = rand::thread_rng().gen_range(0..this.accounts.len());
        let from = this
            .accounts
            .get(index)
            .expect("no account with this index found")
            .to_owned();

        let connection = Api::<sr25519::Pair, _>::new(client)
            .expect("Connection could not be established")
            .set_signer(from);

        let index = rand::thread_rng().gen_range(0..this.accounts.len());
        let to = AccountId::from(
            this.accounts
                .get(index)
                .expect("no account with this index found")
                .to_owned()
                .public(),
        );

        loop {
            let transfer_value = 1u128;
            let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
                connection,
                "Balances",
                "transfer",
                GenericAddress::Id(to.clone()),
                Compact(transfer_value)
            );

            // XtStatus::

            let tx_hash = connection
                .send_extrinsic(tx.hex_encode(), XtStatus::InBlock)
                .unwrap()
                .expect("Could not get tx hash");

            debug!("[+] Transaction hash: {}", tx_hash);

            let mut counter = this.counter.lock().unwrap();

            debug!("id {}, counter: {}", this.id, *counter);

            if
            // *counter >= this.threshold
            *counter >= cmp::max(this.threshold, this.total)
            // && tock >= this.until
            {
                break;
            } else {
                *counter += 1;
            }
        }

        Poll::Ready(())
    }
}
