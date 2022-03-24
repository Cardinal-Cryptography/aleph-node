use crate::Connection;
use substrate_api_client::{AccountId, Balance};

pub fn get_free_balance(connection: &Connection, account: &AccountId) -> Balance {
    match connection
        .get_account_data(account)
        .expect("Should be able to access account data")
    {
        Some(account_data) => account_data.free,
        // Account may have not been initialized yet or liquidated due to the lack of funds.
        None => 0,
    }
}

pub fn locks(
    connection: &Connection,
    account: &AccountId,
) -> Option<Vec<pallet_balances::BalanceLock<Balance>>> {
    connection
        .get_storage_map("Balances", "Locks", account, None)
        .expect("Key `Locks` should be present in storage")
}
