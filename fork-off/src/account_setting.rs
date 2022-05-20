use std::collections::HashMap;

use codec::Encode;
use log::info;
use serde::Deserialize;

use crate::{
    hashing::{combine_storage_keys, hash_account, hash_storage_prefix},
    AccountId, Balance, Storage, StoragePath, StorageValue,
};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Encode)]
pub struct AccountData {
    pub free: Balance,
    pub reserved: Balance,
    pub misc_frozen: Balance,
    pub fee_frozen: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Encode)]
pub struct AccountInfo {
    pub nonce: u32,
    pub consumers: u32,
    pub providers: u32,
    pub sufficients: u32,
    pub data: AccountData,
}

impl AccountInfo {
    pub fn to_storage_value(&self) -> StorageValue {
        format!("0x{}", hex::encode(Encode::encode(self)))
    }
}

pub type AccountSetting = HashMap<AccountId, AccountInfo>;

fn get_account_map() -> StoragePath {
    StoragePath("System.Account".to_string())
}

pub fn apply_account_setting(mut state: Storage, setting: AccountSetting) -> Storage {
    let account_map = hash_storage_prefix(&get_account_map());
    for (account, info) in setting {
        let account_hash = hash_account(&account);
        let key = combine_storage_keys(&account_map, &account_hash);

        state.insert(key, info.to_storage_value());
        info!("Account info of `{:?}` set to `{:?}`", account, info);
    }
    state
}
