use crate::{state_query_storage_at, Connection};
use codec::Decode;
use pallet_balances::BalanceLock;
use substrate_api_client::{utils::FromHexString, AccountId, Balance};

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

pub fn locks(connection: &Connection, accounts: &[AccountId]) -> Vec<Vec<BalanceLock<Balance>>> {
    let storage_keys = accounts
        .into_iter()
        .map(|account| {
            connection
                .metadata
                .storage_map_key("Balances", "Locks", account)
                .expect(&format!(
                    "Cannot create storage key for account {}!",
                    account
                ))
        })
        .collect::<Vec<_>>();
    let storage_entries = match state_query_storage_at(&connection, storage_keys) {
        Ok(storage_entries) => storage_entries
            .into_iter()
            .map(|storage_entry| {
                let entry_bytes = Vec::from_hex(storage_entry.expect("Storage entry is null!"))
                    .expect("Cannot parse hex string!");
                let balance_lock: Vec<pallet_balances::BalanceLock<Balance>> =
                    Decode::decode(&mut entry_bytes.as_slice())
                        .expect("Failed to decode locked balances!");
                balance_lock
            })
            .collect::<Vec<Vec<_>>>(),
        Err(err) => {
            panic!("Failed to query storage, details: {}", &err[..]);
        }
    };
    storage_entries
}
