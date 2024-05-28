use substrate_prometheus_endpoint::{
    register, Gauge, Histogram, HistogramOpts, PrometheusError, Registry, U64,
};

use crate::{block::TreePathAnalyzer, BlockId};

pub enum BestBlockRelatedMetrics<TPA> {
    Prometheus {
        top_finalized_block: Gauge<U64>,
        best_block: Gauge<U64>,
        reorgs: Histogram,
        best_block_id: BlockId,
        tree_path_analyzer: TPA,
    },
    Noop,
}

impl<TPA: TreePathAnalyzer> BestBlockRelatedMetrics<TPA> {
    pub fn new(
        registry: Option<Registry>,
        tree_path_analyzer: TPA,
    ) -> Result<Self, PrometheusError> {
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
            best_block_id: (Default::default(), 0u32).into(),
            tree_path_analyzer,
        })
    }

    pub fn report_best_block_imported(&self, block_id: BlockId) {
        if let Self::Prometheus {
            best_block,
            reorgs,
            tree_path_analyzer,
            best_block_id,
            ..
        } = self
        {
            best_block.set(block_id.number() as u64);
            match tree_path_analyzer
                .retracted_path_length(&(block_id.hash(), block_id.number()).into(), best_block_id)
            {
                Ok(0) => {}
                Ok(reorg_len) => {
                    reorgs.observe(reorg_len as f64);
                }
                Err(e) => {
                    log::debug!("Failed to calculate reorg length: {:?}", e);
                }
            }
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

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use super::*;
    use crate::{
        block::UnverifiedHeader,
        testing::{
            client_chain_builder::ClientChainBuilder,
            mocks::{TestClient, TestClientBuilder, TestClientBuilderExt},
        },
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

    impl TreePathAnalyzer for TestClient {
        type Error = sp_blockchain::Error;

        fn lowest_common_ancestor(&self, a: &BlockId, b: &BlockId) -> Result<BlockId, Self::Error> {
            sp_blockchain::lowest_common_ancestor(self, a.hash(), b.hash())
                .map(|id| BlockId::new(id.hash, id.number))
        }
    }

    #[tokio::test]
    async fn test_reorg_detection() {
        let client = Arc::new(TestClientBuilder::new().build());
        let client_builder = Arc::new(TestClientBuilder::new().build());
        let mut chain_builder = ClientChainBuilder::new(client.clone(), client_builder);

        let a = chain_builder
            .build_and_import_branch_above(&chain_builder.genesis_hash(), 5)
            .await;
        let b = chain_builder
            .build_and_import_branch_above(&a[0].header.hash(), 3)
            .await;
        let c = chain_builder
            .build_and_import_branch_above(&a[2].header.hash(), 2)
            .await;

        //                  - C0 - C1
        //                /
        // G - A0 - A1 - A2 - A3 - A4
        //      \
        //        - B0 - B1 - B2

        for (prev, new_best, expected) in [
            (&a[1], &a[2], Some(0)),
            (&a[1], &a[4], Some(0)),
            (&a[1], &a[1], Some(0)),
            (&a[2], &b[0], Some(2)),
            (&b[0], &a[2], Some(1)),
            (&c[1], &b[2], Some(4)),
        ] {
            assert_eq!(
                client
                    .retracted_path_length(&prev.header.id(), &new_best.header.id())
                    .ok(),
                expected,
            );
        }
    }
}
