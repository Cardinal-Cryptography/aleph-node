use std::{collections::HashMap, str::FromStr};

use crate::Storage;
use codec::Encode;
use log::info;
use serde::Deserialize;

use crate::types::{AccountId, Balance, StorageKey, StoragePath, StorageValue};

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

impl From<AccountInfo> for StorageValue {
    fn from(account_info: AccountInfo) -> StorageValue {
        StorageValue::new(&hex::encode(Encode::encode(&account_info)))
    }
}

pub type AccountSetting = HashMap<AccountId, AccountInfo>;

fn get_account_map() -> StoragePath {
    StoragePath::from_str("System.Account").unwrap()
}

pub fn apply_account_setting(mut state: Storage, setting: AccountSetting) -> Storage {
    let account_map: StorageKey = get_account_map().into();
    for (account, info) in setting {
        let account_hash = account.clone().into();
        let key = &account_map.join(&account_hash);

        state.insert(key.clone(), info.clone().into());
        info!("Account info of `{:?}` set to `{:?}`", account, info);
    }
    state
}
