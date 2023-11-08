mod chain_status;
mod timing;

pub use timing::{Checkpoint, TimingBlockMetrics};
pub(crate) const LOG_TARGET: &str = "aleph-metrics";
