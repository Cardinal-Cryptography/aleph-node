mod config;

use clap::Parser;
use codec::Compact;
use config::Config;
use futures::future::join_all;
use futures::Future;
use hdrhistogram::Histogram as HdrHistogram;
use log::{debug, info};
use rand::Rng;
use sp_core::{sr25519, Pair};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;
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

    let mut tasks = Vec::with_capacity(config.concurrency);

    let concurrency: u64 = config.concurrency as u64;
    let n_transactions = config.throughput * config.duration;
    let batch = n_transactions / concurrency;

    let accounts = config::accounts(config.base_path, config.account_ids, config.key_filename);

    let histogram = Arc::new(Mutex::new(
        HdrHistogram::<u64>::new_with_bounds(1, u64::max_value(), 3).unwrap(),
    ));

    for id in 0..config.concurrency {
        let histogram = Arc::clone(&histogram);
        let accounts = accounts.clone();
        let url = format!("ws://{}:{}", config.host, config.port);

        tasks.push(tokio::spawn(async move {
            let client = Client::new(id, batch, /*duration*/ url, accounts, histogram);
            client.await;
        }));
    }

    // NOTE: we start measuring time after all the configuration is done
    let tick = Instant::now();
    join_all(tasks).await;
    let tock = tick.elapsed().as_millis();

    let histogram = histogram.lock().unwrap();
    println!("Summary:\n Transactions sent:   {}\n Total time:      {} ms\n Slowest tx:    {} ms\n Fastest tx:    {} ms\n Average:    {:.1} ms\n Throughput: {:.1} tx/s",
             histogram.len (),
             tock,
             histogram.max (),
             histogram.min (),
             histogram.mean (),
             1000.0 * histogram.len () as f64 / tock as f64
    );

    Ok(())
}

struct Client {
    /// client id
    id: usize,
    /// how many tx to make
    batch: u64,
    /// URL for ws connection
    url: String,
    /// accounts for signing tx and sendig them to
    accounts: Vec<sr25519::Pair>,
    /// thread shared, thread safe histogram
    histogram: Arc<Mutex<HdrHistogram<u64>>>,
}

impl Client {
    fn new(
        id: usize,
        batch: u64,
        url: String,
        accounts: Vec<sr25519::Pair>,
        histogram: Arc<Mutex<HdrHistogram<u64>>>,
    ) -> Self {
        Self {
            id,
            batch,
            url,
            accounts,
            histogram,
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

        let mut counter = 0;
        let mut nonce = connection.get_nonce().unwrap();

        loop {
            let transfer_value = 1u128;

            let call = compose_call!(
                connection.metadata,
                "Balances",
                "transfer",
                GenericAddress::Id(to.clone()),
                Compact(transfer_value)
            );

            let tx: UncheckedExtrinsicV4<_> = compose_extrinsic_offline!(
                connection.clone().signer.unwrap(),
                call,
                nonce,
                Era::Immortal,
                connection.genesis_hash,
                connection.genesis_hash,
                connection.runtime_version.spec_version,
                connection.runtime_version.transaction_version
            );

            let start_time = Instant::now();

            let block_hash = connection
                .send_extrinsic(tx.hex_encode(), XtStatus::Ready)
                .expect("Could not send transaction");

            let elapsed_time = start_time.elapsed().as_millis();

            let mut hist = this.histogram.lock().unwrap();
            *hist += elapsed_time as u64;

            debug!(
                "Client id {}, sent {} txs, sending account nonce: {}, last block hash {:?}, last tx elapsed time {}",
                this.id, counter, nonce, block_hash, elapsed_time
            );

            if counter >= this.batch {
                break;
            } else {
                counter += 1;
                nonce += 1;
            }
        }

        Poll::Ready(())
    }
}
