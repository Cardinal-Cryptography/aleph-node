use std::{future, num::NonZeroUsize, time::Instant};

use futures::StreamExt;
use lru::LruCache;
use sc_client_api::{BlockBackend, ImportNotifications};
use sc_transaction_pool::{BasicPool, ChainApi};
use sc_transaction_pool_api::{
    error::{Error, IntoPoolError},
    ImportNotificationStream, TransactionPool,
};
use sp_api::BlockT;
use sp_runtime::traits;
use tokio::select;

const LOG_TARGET: &str = "aleph-metrics";

// Size of transaction cache: 32B (Hash) + 16B (Instant) * `100_000` is approximately 4.8MB
const TRANSACTION_CACHE_SIZE: usize = 100_000;
// Maximum number of transactions to recheck if they are still in the pool, per single loop iteration.
const MAX_RECHECKED_TRANSACTIONS: usize = 4;

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
        NonZeroUsize::new(TRANSACTION_CACHE_SIZE).expect("the cache size is a non-zero constant"),
    );

    loop {
        select! {
            maybe_block = best_block_notifications.next() => {
                match maybe_block {
                    Some(block) => {
                        if let Ok(Some(body)) = backend.block_body(block.hash) {
                            for xt in body {
                                let hash = pool.hash_of(&xt);
                                if let Some(insert_time) = cache.pop(&hash) {
                                    let elapsed = insert_time.elapsed();
                                    log::trace!(target: LOG_TARGET, "[transaction_pool_metrics] extrinsic {hash:?} included after {elapsed:?}, lru size = {:?}", cache.len());
                                }
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
                        // Putting new transaction can evict the oldest one. However, even if the
                        // removed transaction was actually still in the pool, we don't have
                        // any guarantees that it could be included in the block. Therefore, we
                        // we ignore such transaction.
                        cache.put(hash, Instant::now());
                    }
                    None => {
                        log::warn!(target: LOG_TARGET, "Transaction stream ended unexpectedly");
                    }
                }
            },
        }
        let mut rechecked_transactions = 0;
        while let Some((&hash, _)) = cache.peek_lru() {
            if !is_in_the_pool(&hash, pool) {
                cache.pop_lru();
            } else {
                cache.promote(&hash);
            }
            rechecked_transactions += 1;
            if rechecked_transactions > MAX_RECHECKED_TRANSACTIONS {
                break;
            }
        }
    }
}

fn is_in_the_pool<A, B>(hash: &ExtrinsicHash<A>, pool: &BasicPool<A, B>) -> bool
where
    B: BlockT,
    A: ChainApi<Block = B> + 'static,
{
    let knowledge = pool.pool().validated_pool().check_is_known(hash, false);
    matches!(
        knowledge.map_err(|e| e.into_pool_error()),
        Err(Ok(Error::AlreadyImported(_)))
    )
}
