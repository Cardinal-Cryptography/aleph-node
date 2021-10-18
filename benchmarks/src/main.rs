mod config;

use clap::Parser;
use config::Config;
use futures::future::join_all;
use futures::{Future, Stream};
use log::info;
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
    info!("Running with config {:#?}", &config);

    let mut tasks = Vec::with_capacity(config.concurrency);
    let counter = Arc::new(Mutex::new(0));
    // let timer = Arc::new(Mutex::new(0u64));
    let tick = Instant::now();

    for id in 0..config.concurrency {
        let count = Arc::clone(&counter);
        let concurrency: u64 = config.concurrency as u64;
        let n_transactions = config.n_transactions;

        let threshold = n_transactions
            .checked_div(concurrency)
            .expect("checked division overflow");

        // let timer = Arc::clone(&timer);
        let until = config.time;

        tasks.push(tokio::spawn(async move {
            let client = Client::new(
                id, count, threshold, tick, // timer,
                until,
            );
            client.await;
        }));
    }

    join_all(tasks).await;

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
    // /// shared timer
    // timer: Arc<Mutex<u64>>,
    /// keep sending requests untill that many seconds elapse
    until: u64,
}

impl Client {
    fn new(
        id: usize,
        counter: Arc<Mutex<u64>>,
        threshold: u64,
        tick: Instant,
        // timer: Arc<Mutex<u64>>,
        until: u64,
    ) -> Self {
        Self {
            id,
            counter,
            threshold,
            tick,
            // timer,
            until,
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

            info!(
                "id {}, counter: {}, elapsed time {}",
                this.id, *counter, tock
            );

            if *counter >= this.threshold && tock >= this.until {
                break;
            } else {
                *counter += 1;
            }
        }

        Poll::Ready(())
    }
}
