mod config;

use clap::Parser;
use config::Config;
use futures::future::join_all;
use futures::{Future, Stream};
use log::{debug, info};
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
    // let timer = Arc::new(Mutex::new(0u64));
    let tick = Instant::now();

    for id in 0..config.concurrency {
        let counter = Arc::clone(&counter);
        let concurrency: u64 = config.concurrency as u64;
        // number of tx we need to send to reach theoretical throughput
        let n_transactions = config.throughput * config.duration;

        let threshold = n_transactions / concurrency;

        // let until = config.length;

        tasks.push(tokio::spawn(async move {
            let client = Client::new(id, counter, threshold, tick, n_transactions);
            client.await;
        }));
    }

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
    /// timer initiated at this clock value
    tick: Instant,
    /// total number of txs to send
    total: u64,
}

impl Client {
    fn new(id: usize, counter: Arc<Mutex<u64>>, threshold: u64, tick: Instant, total: u64) -> Self {
        Self {
            id,
            counter,
            threshold,
            tick,
            total,
        }
    }
}

impl Future for Client {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        loop {
            let this = &mut *self;
            let mut counter = this.counter.lock().unwrap();
            // NOTE: each client has it's own clock, do we care?
            let tock = this.tick.elapsed().as_secs();

            debug!(
                "id {}, counter: {}, elapsed time {}",
                this.id, *counter, tock
            );

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
