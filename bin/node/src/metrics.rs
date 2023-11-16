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
                        for xt in backend.block(block.hash).unwrap().unwrap().block.extrinsics() {
                            let hash = pool.hash_of(xt);
                            if let Some(insert_time) = cache.pop(&hash) {
                                let elapsed = insert_time.elapsed();
                                log::trace!(target: LOG_TARGET, "[transaction_pool_metrics] extrinsic {hash:?} included after {elapsed:?}, lru size = {:?}", cache.len());
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
                        let maybe_popped = cache.put(hash, Instant::now());
                        log::trace!(target: LOG_TARGET, "[transaction_pool_metrics] inserted extrinsic {hash:?}, lru size = {:?}", cache.len());
                        if let Some(_insert_time) = maybe_popped {
                            // TODO: check if still in validated pool and maybe report
                        }
                    }
                    None => {
                        log::warn!(target: LOG_TARGET, "Tx stream ended unexpectedly");
                    }
                }
            },
        }
        let mut popped_transactions = 0;
        while let Some((&hash, _)) = cache.peek_lru() {
            match pool
                .pool()
                .validated_pool()
                .check_is_known(&hash, false)
                .map_err(|e| e.into_pool_error())
            {
                Err(Ok(Error::AlreadyImported(_))) => break,
                Ok(()) | Err(Ok(Error::TemporarilyBanned)) => {
                    cache.pop_lru();
                    log::trace!(
                        "[transaction_pool_metrics] Extrinsic {hash:?} is no longer valid, removing, lru size = {:?}",
                        cache.len()
                    );
                }
                _ => {
                    log::trace!("Unknown error in from transaction pool check_is_known");
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
