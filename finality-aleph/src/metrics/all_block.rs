use primitives::BlockHash;

use super::{timing::DefaultClock, Checkpoint};
use crate::TimingBlockMetrics;

#[derive(Clone)]
pub struct AllBlockMetrics {
    timing_metrics: TimingBlockMetrics<DefaultClock>,
}

impl AllBlockMetrics {
    pub fn new(timing_metrics: TimingBlockMetrics<DefaultClock>) -> Self {
        AllBlockMetrics { timing_metrics }
    }

    pub fn report_block(&self, hash: BlockHash, checkpoint: Checkpoint) {
        self.timing_metrics.report_block(hash, checkpoint);
    }
}
