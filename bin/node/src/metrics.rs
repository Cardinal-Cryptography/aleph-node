use futures::StreamExt;
use log::warn;
use lru::LruCache;
use parity_scale_codec::Encode;
use sc_client_api::{BlockBackend, ImportNotifications};
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::ImportNotificationStream;
use sp_api::BlockT;
use std::fmt::Debug;
use std::future;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::time::Instant;
use tokio::select;

const MAX_TRANSACTIONS_PER_CHECKPOINT: usize = 100000;
const LOG_TARGET: &str = "aleph-metrics";
pub async fn run_metrics<A, B, BE, EH>(
    transaction_notifications: ImportNotificationStream<EH>,
    import_notifications: ImportNotifications<B>,
    backend: &BE,
    api: &A,
) where
    B: BlockT,
    A: ChainApi<Block = B>,
    BE: BlockBackend<B>,
    EH: Hash + Eq + Clone + Debug + From<[u8; 32]>,
{
    let mut best_block_notifications = import_notifications
        .fuse()
        .filter(|notification| future::ready(notification.is_new_best));
    let mut transaction_notifications = transaction_notifications.fuse();

    let mut cache: LruCache<EH, Instant> = LruCache::new(
        NonZeroUsize::new(MAX_TRANSACTIONS_PER_CHECKPOINT).expect("LRU cache has non zero size"),
    );

    loop {
        select! {
            maybe_block = best_block_notifications.next() => {
                match maybe_block {
                    Some(block) => {
                        for xt in backend.block(block.hash).unwrap().unwrap().block.extrinsics() {
                            let hash = sp_io::hashing::blake2_256(&xt.encode()).into();
                            if let Some(insert_time) = cache.pop(&hash) {
                                let elapsed = insert_time.elapsed();
                                log::info!(target: LOG_TARGET, "[gqfrdsf] extrinsic {hash:?} included in {elapsed:?}");
                            }
                        }
                    }
                    None => {
                        warn!(target: LOG_TARGET, "Import notification stream ended unexpectedly");
                    }
                }
            },
            maybe_transaction = transaction_notifications.next() => {
                match maybe_transaction {
                    Some(hash) => {
                        log::info!(target: LOG_TARGET, "[gqfrdsf] insert extrinsic {hash:?}, lru size = {:?}", cache.len());
                        cache.put(hash, Instant::now());
                    }
                    None => {
                        warn!(target: LOG_TARGET, "Tx stream ended unexpectedly");
                    }
                }
            },
        }
    }
}
