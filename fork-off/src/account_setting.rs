use std::{collections::HashMap, str::FromStr};

use crate::Storage;
use codec::{Decode, Encode};
use log::info;
use serde::{Deserialize, Deserializer};

use crate::types::{AccountId, Balance, StorageKey, StoragePath, StorageValue};

use frame_system::AccountInfo as SubstrateAccountInfo;
use pallet_balances::AccountData as SubstrateAccountData;
use serde::de::Error;

/// Deserializable `AccountData`.
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Default)]
pub struct AccountData(SubstrateAccountData<Balance>);

impl<'de> Deserialize<'de> for AccountData {
    fn deserialize<D>(deserializer: D) -> Result<AccountData, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        serde_json::from_str(s).map_err(Error::custom)
    }
}

/// Deserializable `AccountInfo`.
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Default)]
pub struct AccountInfo(SubstrateAccountInfo<u32, AccountData>);

impl<'de> Deserialize<'de> for AccountInfo {
    fn deserialize<D>(deserializer: D) -> Result<AccountInfo, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        serde_json::from_str(s).map_err(Error::custom)
    }
}

impl From<AccountInfo> for StorageValue {
    fn from(account_info: AccountInfo) -> StorageValue {
        StorageValue::new(&hex::encode(Encode::encode(&account_info)))
    }
}

/// Create `AccountInfo` with all parameters set to `0` apart from free balances, which is
/// set to `free` and number of providers, which is set to `1`.
pub fn account_info_from_free(free: Balance) -> AccountInfo {
    AccountInfo(SubstrateAccountInfo {
        providers: 1,
        data: AccountData(SubstrateAccountData {
            free,
            ..SubstrateAccountData::default()
        }),
        ..SubstrateAccountInfo::default()
    })
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
