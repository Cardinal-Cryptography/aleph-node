use crate::{read_storage, AnyConnection};
use primitives::Balance;

/// Reads from the storage how much balance is currently on chain.
///
/// Performs a single storage read.
pub fn total_issuance<C: AnyConnection>(connection: &C) -> Balance {
    read_storage(connection, "Balances", "TotalIssuance")
}
