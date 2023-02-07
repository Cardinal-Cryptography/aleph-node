mod history;
mod scheduler;
mod staking;
mod transaction_payment;

pub use history::MigrateToV1 as HistoryMigrateToV1;
pub use scheduler::MigrateToV3 as SchedulerMigrateToV3;
pub use staking::BumpStorageVersionFromV7ToV10;
pub use transaction_payment::MigrateToV2 as TransactionPaymentMigrateToV2;
