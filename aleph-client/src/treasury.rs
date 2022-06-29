use frame_support::PalletId;
use log::info;
use sp_runtime::{traits::AccountIdConversion, AccountId32};

use primitives::Balance;

use crate::AnyConnection;

/// Returns the account of the treasury.
pub fn treasury_account() -> AccountId32 {
    PalletId(*b"a0/trsry").into_account_truncating()
}

/// Returns how many treasury proposals have ever been created.
///
/// Requires a single storage read.
pub fn proposals_counter<C: AnyConnection>(connection: &C) -> u32 {
    connection
        .as_connection()
        .get_storage_value("Treasury", "ProposalCount", None)
        .expect("Key `Treasury::ProposalCount` should be present in storage")
        .unwrap_or(0)
}

/// Calculates how much balance will be paid out to the treasury after each era.
///
/// Every call to this method is potentially expensive - it requires three storage reads.
pub fn staking_treasury_payout<C: AnyConnection>(connection: &C) -> Balance {
    let sessions_per_era = connection
        .as_connection()
        .get_constant::<u32>("Staking", "SessionsPerEra")
        .expect("Constant `Staking::SessionsPerEra` should be present");
    let session_period = connection
        .as_connection()
        .get_constant::<u32>("Elections", "SessionPeriod")
        .expect("Constant `Elections::SessionPeriod` should be present");
    let millisecs_per_block = 2 * connection
        .as_connection()
        .get_constant::<u64>("Timestamp", "MinimumPeriod")
        .expect("Constant `Timestamp::MinimumPeriod` should be present");

    let millisecs_per_era = millisecs_per_block * session_period as u64 * sessions_per_era as u64;
    let treasury_era_payout_from_staking = primitives::staking::era_payout(millisecs_per_era).1;
    info!(
        "[+] Possible treasury gain from staking is {}",
        treasury_era_payout_from_staking
    );
    treasury_era_payout_from_staking
}
