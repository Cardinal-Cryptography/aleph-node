use primitives::{Block, BlockNumber, Header, HeaderT};
use sp_blockchain::{lowest_common_ancestor, HeaderMetadata};
use substrate_prometheus_endpoint::{
    register, Gauge, Histogram, HistogramOpts, PrometheusError, Registry, U64,
};

use crate::BlockId;

pub enum BestBlockRelatedMetrics<R> {
    Prometheus {
        top_finalized_block: Gauge<U64>,
        best_block: Gauge<U64>,
        reorgs: Histogram,
        best_block_header: Header,
        reorg_detector: R,
    },
    Noop,
}

impl<R: ReorgDetector> BestBlockRelatedMetrics<R> {
    pub fn new(registry: Option<Registry>, reorg_detector: R) -> Result<Self, PrometheusError> {
        let registry = match registry {
            Some(registry) => registry,
            None => return Ok(Self::Noop),
        };

        Ok(Self::Prometheus {
            top_finalized_block: register(
                Gauge::new("aleph_top_finalized_block", "no help")?,
                &registry,
            )?,
            best_block: register(Gauge::new("aleph_best_block", "no help")?, &registry)?,
            reorgs: register(
                Histogram::with_opts(
                    HistogramOpts::new("aleph_reorgs", "Number of reorgs by length")
                        .buckets(vec![1., 2., 4., 9.]),
                )?,
                &registry,
            )?,
            best_block_header: Header::new(
                0,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
            reorg_detector,
        })
    }

    pub fn report_best_block_imported(&self, header: Header) {
        if let Self::Prometheus {
            best_block,
            reorgs,
            reorg_detector,
            best_block_header,
            ..
        } = self
        {
            if let Some(reorg_len) =
                reorg_detector.retracted_path_length(&header, best_block_header)
            {
                reorgs.observe(reorg_len as f64);
            }
            best_block.set(*header.number() as u64)
        }
    }

    pub fn report_block_finalized(&self, block_id: BlockId) {
        if let Self::Prometheus {
            top_finalized_block,
            ..
        } = self
        {
            // Sometimes finalization can also cause best block update. However,
            // RPC best block subscription won't notify about that immediately, so
            // we also don't update there. Also in that case, substrate sets best_block to
            // the newly finalized block (see test), so the best block will be updated
            // after importing anything on the newly finalized branch.
            top_finalized_block.set(block_id.number() as u64);
        }
    }
}

pub trait ReorgDetector {
    fn retracted_path_length(&self, from: &Header, to: &Header) -> Option<BlockNumber>;
}

struct ReorgDetectorImpl<'a, BE: HeaderMetadata<Block>> {
    backend: &'a BE,
}

impl<'a, BE: HeaderMetadata<Block>> ReorgDetector for ReorgDetectorImpl<'a, BE> {
    fn retracted_path_length(&self, from: &Header, to: &Header) -> Option<BlockNumber> {
        if from.hash() == to.hash() || from.hash() == *to.parent_hash() {
            // Quit early when no change or the best is a child of the previous best.
            return None;
        }
        let lca = lowest_common_ancestor(self.backend, from.hash(), to.hash()).ok()?;
        let len = from.number().saturating_sub(lca.number);
        if len == 0 {
            return None;
        }
        Some(len)
    }
}

#[cfg(test)]
mod test {

    use std::{collections::HashMap, sync::Arc};

    use futures::{FutureExt, Stream};
    use parity_scale_codec::Encode;
    use sc_block_builder::BlockBuilderBuilder;
    use sc_client_api::{BlockchainEvents, HeaderBackend};
    use substrate_test_runtime_client::AccountKeyring;

    use super::*;
    use crate::testing::{
        client_chain_builder::ClientChainBuilder,
        mocks::{TBlock, THash, TestClient, TestClientBuilder, TestClientBuilderExt},
    };

    #[tokio::test]
    async fn when_finalizing_with_reorg_best_block_is_set_to_that_finalized_block() {
        let client = Arc::new(TestClientBuilder::new().build());
        let client_builder = Arc::new(TestClientBuilder::new().build());
        let mut chain_builder = ClientChainBuilder::new(client.clone(), client_builder);

        let chain_a = chain_builder
            .build_and_import_branch_above(&chain_builder.genesis_hash(), 5)
            .await;

        // (G) - A1 - A2 - A3 - A4 - A5;

        assert_eq!(
            client.chain_info().finalized_hash,
            chain_builder.genesis_hash()
        );
        assert_eq!(client.chain_info().best_number, 5);

        let chain_b = chain_builder
            .build_and_import_branch_above(&chain_a[0].header.hash(), 3)
            .await;
        chain_builder.finalize_block(&chain_b[0].header.hash());

        // (G) - (A1) - A2 - A3 - A4 - A5
        //         \
        //          (B2) - B3 - B4

        assert_eq!(client.chain_info().best_number, 2);
        assert_eq!(client.chain_info().finalized_hash, chain_b[0].header.hash());
    }
}
