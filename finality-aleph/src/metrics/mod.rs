mod chain_state;
mod timing;
pub mod transaction_pool;

pub use chain_state::run_chain_state_metrics;
use sp_runtime::traits::Member;
pub use timing::{exponential_buckets_two_sided, Checkpoint, TimingBlockMetrics};
const LOG_TARGET: &str = "aleph-metrics";

#[async_trait::async_trait]
pub trait TransactionPoolInfoProvider {
    type TxHash: Member + std::hash::Hash;
    type Extrinsic;
    async fn next_transaction(&mut self) -> Option<Self::TxHash>;

    fn hash_of(&self, extrinsic: &Self::Extrinsic) -> Self::TxHash;

    fn pool_contains(&self, transaction: &Self::TxHash) -> bool;
}
