use std::{collections::HashMap, sync::Arc};

use crate::{jsonrpc_client::Client, Storage};
use anyhow::Result;
use futures::future::join_all;
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
        input: Arc<Mutex<Vec<StorageKey>>>,
        output: Arc<Mutex<Storage>>,
    ) {
        const LOG_PROGRESS_FREQUENCY: usize = 500;
        let next_input = || input.lock().pop();

        while let Some(key) = next_input() {
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

    async fn get_values(
        &self,
        keys: Vec<StorageKey>,
        block_hash: BlockHash,
        num_workers: u32,
    ) -> Storage {
        let n_keys = keys.len();
        let input = Arc::new(Mutex::new(keys));
        let output = Arc::new(Mutex::new(HashMap::with_capacity(n_keys)));
        let mut workers = Vec::new();

        for _ in 0..(num_workers as usize) {
            workers.push(self.value_fetching_worker(
                block_hash.clone(),
                input.clone(),
                output.clone(),
            ));
        }
        info!("Started {} workers to download values.", workers.len());
        join_all(workers).await;
        assert!(input.lock().is_empty(), "Not all keys were fetched");
        let mut guard = output.lock();
        std::mem::take(&mut guard)
    }

    pub async fn get_full_state_at_best_block(&self, num_workers: u32) -> Storage {
        let best_block = self.client.best_block().await.unwrap();
        info!("Fetching state at block {:?}", best_block);

        let keys = self.client.all_keys(&best_block).await.unwrap();
        info!("Found {} keys and", keys.len());

        self.get_values(keys, best_block, num_workers).await
    }
}
