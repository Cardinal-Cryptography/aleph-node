use std::fmt::Debug;

use derive_more::Display;
use frame_support::Hashable;
use futures::{channel::mpsc, Stream, StreamExt};
use log::warn;
use parity_scale_codec::Encode;
use primitives::Block;
use sc_client_api::{FinalityNotifications, ImportNotifications};
use sc_transaction_pool_api::ImportNotificationStream as PoolImportNotificationStream;
use sp_consensus::BlockOrigin;
use sp_core::H256;
use sp_runtime::{
    traits,
    traits::{Block as _, HashingFor},
    OpaqueExtrinsic,
};
use substrate_prometheus_endpoint::{PrometheusError, Registry};

use super::{finality_rate::FinalityRateMetrics, timing::DefaultClock, Checkpoint};
use crate::{
    block::{
        BlockchainEvents, ChainStatus, ChainStatusNotifier, Header, TreePathAnalyzer,
        UnverifiedHeader,
    },
    metrics::{
        best_block_related_metrics::BestBlockRelatedMetrics,
        transaction_pool::TransactionPoolMetrics, LOG_TARGET,
    },
    BlockId, SubstrateChainStatus, TimingBlockMetrics,
};

#[derive(Debug, Display)]
pub enum Error {
    BlockImportStreamClosed,
    FinalityNotificationStreamClosed,
    TransactionPoolStreamClosed,
}

pub async fn run_metrics_service<TS: Stream<Item = TxHash> + Unpin>(
    metrics: &SloMetrics,
    import_notifications: &mut ImportNotifications<Block>,
    finality_notifications: &mut FinalityNotifications<Block>,
    transaction_pool_stream: &mut TS,
) -> Result<(), Error> {
    loop {
        tokio::select! {
            maybe_block = import_notifications.next() => {
                let block = maybe_block.ok_or(Error::BlockImportStreamClosed)?;
                metrics.report_block_imported((block.hash, block.header.number).into(), block.is_new_best, block.origin == BlockOrigin::Own);
            },
            maybe_block = finality_notifications.next() => {
                let block = maybe_block.ok_or(Error::FinalityNotificationStreamClosed)?;
                metrics.report_block_finalized((block.hash, block.header.number).into());
            },

            maybe_tx = transaction_pool_stream.next() => {
                let tx = maybe_tx.ok_or(Error::TransactionPoolStreamClosed)?;
                metrics.report_transaction_in_pool(tx);
            },
        }
    }
}

pub type Hashing = traits::HashingFor<Block>;
pub type TxHash = <Hashing as sp_runtime::traits::Hash>::Output;

pub struct SloMetrics {
    all_block_metrics: AllBlockMetrics,
    transaction_metrics: TransactionPoolMetrics<TxHash>,
    best_block_related_metrics: BestBlockRelatedMetrics<SubstrateChainStatus>,
    chain_status: SubstrateChainStatus,
}

impl SloMetrics {
    pub fn new(registry: Option<&Registry>, chain_status: SubstrateChainStatus) -> Self {
        let warn_creation_failed = |name, e| warn!(target: LOG_TARGET, "Failed to register Prometheus {name} metrics: {e:?}.");
        let all_block_metrics = AllBlockMetrics::new(registry);
        let best_block_related_metrics =
            BestBlockRelatedMetrics::new(registry.cloned(), chain_status.clone()).unwrap_or_else(
                |e| {
                    warn_creation_failed("best block related", e);
                    BestBlockRelatedMetrics::Noop
                },
            );
        let transaction_metrics = TransactionPoolMetrics::new(registry).unwrap_or_else(|e| {
            warn_creation_failed("transaction pool", e);
            TransactionPoolMetrics::Noop
        });

        SloMetrics {
            all_block_metrics,
            best_block_related_metrics,
            transaction_metrics,
            chain_status,
        }
    }

    pub fn all_block_metrics(&self) -> &AllBlockMetrics {
        &self.all_block_metrics
    }

    pub fn report_transaction_in_pool(&self, hash: TxHash) {
        self.transaction_metrics.report_in_pool(hash);
    }

    pub fn report_block_imported(&self, block_id: BlockId, is_new_best: bool, own: bool) {
        if is_new_best {
            self.best_block_related_metrics
                .report_best_block_imported(block_id.clone());
        }
        if let Ok(Some(block)) = self.chain_status.block(block_id.clone()) {
            for xt in block.extrinsics() {
                self.transaction_metrics.report_in_block(
                    xt.using_encoded(|x| <Hashing as sp_runtime::traits::Hash>::hash(x)),
                );
            }
        }
        self.all_block_metrics.report_imported(block_id, own);
    }

    pub fn report_block_finalized(&self, block_id: BlockId) {
        self.best_block_related_metrics
            .report_block_finalized(block_id.clone());
        self.all_block_metrics.report_finalized(block_id);
    }
}

#[derive(Clone)]
pub struct AllBlockMetrics {
    timing_metrics: TimingBlockMetrics<DefaultClock>,
    finality_rate_metrics: FinalityRateMetrics,
}

impl AllBlockMetrics {
    pub fn new(registry: Option<&Registry>) -> Self {
        let timing_metrics = TimingBlockMetrics::new(registry, DefaultClock).unwrap_or_else(|e| {
            warn!(
                target: LOG_TARGET,
                "Failed to register Prometheus block timing metrics: {:?}.", e
            );
            TimingBlockMetrics::Noop
        });
        let finality_rate_metrics = FinalityRateMetrics::new(registry).unwrap_or_else(|e| {
            warn!(
                target: LOG_TARGET,
                "Failed to register Prometheus finality rate metrics: {:?}.", e
            );
            FinalityRateMetrics::Noop
        });
        AllBlockMetrics {
            timing_metrics,
            finality_rate_metrics,
        }
    }

    pub fn report_importing(&self, block_id: BlockId) {
        self.report_checkpoint(block_id, Checkpoint::Importing, None);
    }

    fn report_imported(&self, block_id: BlockId, own: bool) {
        self.report_checkpoint(block_id, Checkpoint::Imported, Some(own));
    }

    pub fn report_proposed(&self, block_id: BlockId) {
        self.report_checkpoint(block_id, Checkpoint::Proposed, None);
    }

    pub fn report_ordered(&self, block_id: BlockId) {
        self.report_checkpoint(block_id, Checkpoint::Ordered, None);
    }

    fn report_finalized(&self, block_id: BlockId) {
        self.report_checkpoint(block_id, Checkpoint::Finalized, None);
    }

    fn report_checkpoint(&self, block_id: BlockId, checkpoint: Checkpoint, is_own: Option<bool>) {
        self.timing_metrics
            .report_block(block_id.hash(), checkpoint);
        self.finality_rate_metrics.report_block(
            block_id.hash(),
            block_id.number(),
            checkpoint,
            None,
        );
    }
}
