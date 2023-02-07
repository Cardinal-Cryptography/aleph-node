mod history;
mod scheduler;
mod staking;

pub use history::MigrateToV1 as HistoryMigrateToV1;
pub use scheduler::MigrateToV3 as SchedulerMigrateToV3;
pub use staking::MigrateToV10 as StakingMigrateToV10;
