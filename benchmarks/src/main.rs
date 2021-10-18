mod config;

use clap::Parser;
use config::Config;
use futures::{Future, Stream};
use log::info;
use std::pin::Pin;
use std::task::{Context, Poll};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_call, compose_extrinsic, AccountId, Api, UncheckedExtrinsicV4, XtStatus,
};

// TODO : tokio runtime that spawn task / core ?
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let config: Config = Config::parse();
    info!("Running with config {:#?}", &config);

    let mut tasks = Vec::with_capacity(config.concurrency);

    for _id in 0..config.concurrency {
        tasks.push(tokio::spawn(async move {
            let client = Client::new();
            client.await;
        }));
    }

    for t in tasks {
        t.await?;
    }

    Ok(())
}

struct Client {}

impl Client {
    fn new() -> Self {
        Client {}
    }
}

impl Future for Client {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            // TODO

            //
            break;
        }

        Poll::Ready(())
    }
}
