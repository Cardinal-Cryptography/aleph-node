use std::fmt::Debug;

use derive_more::Display;
use futures::{Stream, StreamExt};
use log::{info, warn};
use parity_scale_codec::Encode;
use primitives::Block;
use sc_client_api::{FinalityNotifications, ImportNotifications};
use sp_consensus::BlockOrigin;
use sp_runtime::traits::{Block as _, Extrinsic};
use substrate_prometheus_endpoint::Registry;

use super::{finality_rate::FinalityRateMetrics, timing::DefaultClock};
use crate::{
    block::ChainStatus,
    metrics::{
        best_block_related::BestBlockRelatedMetrics, timing::Checkpoint,
        transaction_pool::TransactionPoolMetrics, TimingBlockMetrics, LOG_TARGET,
    },
    BlockId, SubstrateChainStatus,
};

#[derive(Debug, Display)]
pub enum Error {
    BlockImport,
    FinalityNotification,
    TransactionPool,
}

pub async fn run_metrics_service<TS: Stream<Item = TxHash> + Unpin>(
    metrics: &SloMetrics,
    import_notifications: &mut ImportNotifications<Block>,
    finality_notifications: &mut FinalityNotifications<Block>,
    transaction_pool_stream: &mut TS,
) -> Result<(), Error> {
    if metrics.is_noop() {
        info!(target: LOG_TARGET, "Stopping metrics service: metrics were disabled.");
        return Ok(());
    }
    loop {
        tokio::select! {
            maybe_block = import_notifications.next() => {
                let block = maybe_block.ok_or(Error::BlockImport)?;
                metrics.report_block_imported((block.hash, block.header.number).into(), block.is_new_best, block.origin == BlockOrigin::Own);
            },
            maybe_block = finality_notifications.next() => {
                let block = maybe_block.ok_or(Error::FinalityNotification)?;
                metrics.report_block_finalized((block.hash, block.header.number).into());
            },

            maybe_tx = transaction_pool_stream.next() => {
                let tx = maybe_tx.ok_or(Error::TransactionPool)?;
                metrics.report_transaction_in_pool(tx);
            },
        }
    }
}

pub type Hashing = sp_runtime::traits::HashingFor<Block>;
pub type TxHash = <Hashing as sp_runtime::traits::Hash>::Output;

pub struct SloMetrics {
    timing_metrics: TimingBlockMetrics,
    finality_rate_metrics: FinalityRateMetrics,
    best_block_related_metrics: BestBlockRelatedMetrics<SubstrateChainStatus>,
    transaction_metrics: TransactionPoolMetrics<TxHash, DefaultClock>,
    chain_status: SubstrateChainStatus,
}

impl SloMetrics {
    pub fn new(registry: Option<&Registry>, chain_status: SubstrateChainStatus) -> Self {
        let warn_creation_failed = |name, e| warn!(target: LOG_TARGET, "Failed to register Prometheus {name} metrics: {e:?}.");
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
        let best_block_related_metrics =
            BestBlockRelatedMetrics::new(registry.cloned(), chain_status.clone()).unwrap_or_else(
                |e| {
                    warn_creation_failed("best block related", e);
                    BestBlockRelatedMetrics::Noop
                },
            );
        let transaction_metrics = TransactionPoolMetrics::new(registry, DefaultClock)
            .unwrap_or_else(|e| {
                warn_creation_failed("transaction pool", e);
                TransactionPoolMetrics::Noop
            });

        SloMetrics {
            timing_metrics,
            finality_rate_metrics,
            best_block_related_metrics,
            transaction_metrics,
            chain_status,
        }
    }

    pub fn is_noop(&self) -> bool {
        matches!(self.timing_metrics, TimingBlockMetrics::Noop)
            && matches!(self.finality_rate_metrics, FinalityRateMetrics::Noop)
            && matches!(
                self.best_block_related_metrics,
                BestBlockRelatedMetrics::Noop
            )
            && matches!(self.transaction_metrics, TransactionPoolMetrics::Noop)
    }

    pub fn timing_metrics(&self) -> &TimingBlockMetrics {
        &self.timing_metrics
    }

    pub fn report_transaction_in_pool(&self, hash: TxHash) {
        self.transaction_metrics.report_in_pool(hash);
    }

    pub fn report_block_imported(&self, block_id: BlockId, is_new_best: bool, own: bool) {
        self.timing_metrics
            .report_block(block_id.hash(), Checkpoint::Imported);
        if own {
            self.finality_rate_metrics
                .report_own_imported(block_id.clone());
        }
        if is_new_best {
            self.best_block_related_metrics
                .report_best_block_imported(block_id.clone());
        }
        if let Ok(Some(block)) = self.chain_status.block(block_id.clone()) {
            for xt in block.extrinsics() {
                if let Some(true) = xt.is_signed() {
                    self.transaction_metrics.report_in_block(
                        xt.using_encoded(<Hashing as sp_runtime::traits::Hash>::hash),
                    );
                }
            }
        }
    }

    pub fn report_block_finalized(&self, block_id: BlockId) {
        self.timing_metrics
            .report_block(block_id.hash(), Checkpoint::Finalized);
        self.finality_rate_metrics
            .report_finalized(block_id.clone());
        self.best_block_related_metrics
            .report_block_finalized(block_id.clone());
    }
}
