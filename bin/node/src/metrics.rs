use futures::StreamExt;
use lru::LruCache;
use sc_client_api::{BlockBackend, ImportNotifications};
use sc_transaction_pool::{BasicPool, ChainApi};
use sc_transaction_pool_api::error::{Error, IntoPoolError};
use sc_transaction_pool_api::{ImportNotificationStream, TransactionPool};
use sp_api::BlockT;
use sp_runtime::traits;
use std::future;
use std::num::NonZeroUsize;
use std::time::Instant;
use tokio::select;

const MAX_TRANSACTIONS_PER_CHECKPOINT: usize = 100000;
const LOG_TARGET: &str = "aleph-metrics";

pub type ExtrinsicHash<A> = <<A as ChainApi>::Block as traits::Block>::Hash;

pub async fn run_metrics<A, B, BE>(
    transaction_notifications: ImportNotificationStream<ExtrinsicHash<A>>,
    import_notifications: ImportNotifications<B>,
    backend: &BE,
    pool: &BasicPool<A, B>,
) where
    B: BlockT,
    A: ChainApi<Block = B> + 'static,
    BE: BlockBackend<B>,
{
    let mut best_block_notifications = import_notifications
        .fuse()
        .filter(|notification| future::ready(notification.is_new_best));
    let mut transaction_notifications = transaction_notifications.fuse();

    let mut cache: LruCache<ExtrinsicHash<A>, Instant> = LruCache::new(
        NonZeroUsize::new(MAX_TRANSACTIONS_PER_CHECKPOINT).expect("LRU cache has non zero size"),
    );

    loop {
        select! {
            maybe_block = best_block_notifications.next() => {
                match maybe_block {
                    Some(block) => {
                        for xt in backend.block(block.hash).unwrap().unwrap().block.extrinsics() {
                            let hash = pool.hash_of(xt);
                            if let Some(insert_time) = cache.pop(&hash) {
                                let elapsed = insert_time.elapsed();
                                log::info!(target: LOG_TARGET, "[transaction_pool_metrics] extrinsic {hash:?} included in {elapsed:?}, lru size = {:?}", cache.len());
                            }
                        }
                    }
                    None => {
                        log::warn!(target: LOG_TARGET, "Import notification stream ended unexpectedly");
                    }
                }
            },
            maybe_transaction = transaction_notifications.next() => {
                match maybe_transaction {
                    Some(hash) => {
                        cache.put(hash, Instant::now());
                        log::info!(target: LOG_TARGET, "[transaction_pool_metrics] insert extrinsic {hash:?}, lru size = {:?}", cache.len());
                    }
                    None => {
                        log::warn!(target: LOG_TARGET, "Tx stream ended unexpectedly");
                    }
                }
            },
        }
        let mut popped_transactions = 0;
        while let Some((&hash, insert_time)) = cache.peek_lru() {
            match pool
                .pool()
                .validated_pool()
                .check_is_known(&hash, false)
                .map_err(|e| e.into_pool_error())
            {
                Err(Ok(Error::AlreadyImported(_))) => break,
                Ok(()) | Err(Ok(Error::TemporarilyBanned)) => {
                    cache.pop_lru();
                    log::info!(
                        "[transaction_pool_metrics] no longer valid {hash:?}, lru size = {:?}",
                        cache.len()
                    );
                }
                _ => {
                    log::info!("Unknown error in from transaction pool check_is_known");
                    break;
                }
            };
            popped_transactions += 1;
            if popped_transactions > 4 {
                break;
            }
        }
    }
}
