use std::{collections::HashMap, sync::Arc};

use crate::{jsonrpc_client::Client, Storage};
use anyhow::Result;
use async_channel::Receiver;
use futures::{future::join_all, join};
use log::info;
use parking_lot::Mutex;

use crate::types::{BlockHash, StorageKey};

pub struct StateFetcher {
    client: Client,
}

impl StateFetcher {
    pub async fn new(ws_rpc_endpoint: String) -> Result<Self> {
        Ok(StateFetcher {
            client: Client::new(&ws_rpc_endpoint).await.unwrap(),
        })
    }

    async fn value_fetching_worker(
        &self,
        block: BlockHash,
        input: Receiver<StorageKey>,
        output: Arc<Mutex<Storage>>,
    ) {
        const LOG_PROGRESS_FREQUENCY: usize = 500;

        while let Ok(key) = input.recv().await {
            let value = self
                .client
                .get_storage(key.clone(), block.clone())
                .await
                .unwrap();

            let mut output_guard = output.lock();
            output_guard.insert(key, value);
            if output_guard.len() % LOG_PROGRESS_FREQUENCY == 0 {
                info!("Fetched {} values", output_guard.len());
            }
        }
    }

    async fn get_full_state_at_block(&self, block_hash: BlockHash, num_workers: u32) -> Storage {
        info!("Fetching state at block {:?}", block_hash);

        let (input, key_fetcher) = self.client.stream_all_keys(&block_hash);
        let output = Arc::new(Mutex::new(HashMap::new()));
        let mut workers = Vec::new();

        for _ in 0..(num_workers as usize) {
            workers.push(self.value_fetching_worker(
                block_hash.clone(),
                input.clone(),
                output.clone(),
            ));
        }

        info!("Started {} workers to download values.", workers.len());
        let (res, _) = join!(key_fetcher, join_all(workers));
        res.unwrap();

        let mut guard = output.lock();
        std::mem::take(&mut guard)
    }

    pub async fn get_full_state(&self, at_block: Option<BlockHash>, num_workers: u32) -> Storage {
        match at_block {
            None => {
                let best_block = self.client.best_block().await.unwrap();
                self.get_full_state_at_block(best_block, num_workers).await
            }
            Some(block) => self.get_full_state_at_block(block, num_workers).await,
        }
    }
}
