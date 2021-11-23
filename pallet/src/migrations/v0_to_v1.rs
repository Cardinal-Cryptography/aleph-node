use super::*;
use crate::{Config, ValidatorsChange, ValidatorsChangeStorageItem};
use frame_support::log;
// use std::default;
// use sp_std::vec;

use frame_support::{
    storage::{generator::StorageValue, StoragePrefixedMap},
    traits::{
        Get, GetStorageVersion, PalletInfoAccess, StorageVersion,
        STORAGE_VERSION_STORAGE_KEY_POSTFIX,
    },
    weights::Weight,
};

// #[frame_support::pallet]
// mod deprecated {
//     use crate::Config;
//     use frame_support::{
//         pallet_prelude::*,
//         sp_runtime::{traits::OpaqueKeys, RuntimeAppPublic},
//         sp_std,
//     };
//     use frame_system::pallet_prelude::*;

//     // use crate::Trait;
//     // use frame_support::{decl_module, decl_storage};
//     // use sp_std::prelude::*;

//     // decl_storage! {
//     //     trait Store for Module<T: Trait> as Indices {
//     //         /// The next free enumeration set.
//     //         pub NextEnumSet get(fn next_enum_set): T::AccountIndex;

//     //         /// The enumeration sets.
//     //         pub EnumSet get(fn enum_set): map hasher(opaque_blake2_256) T::AccountIndex => Vec<T::AccountId>;
//     //     }
//     // }
//     // decl_module! {
//     //     pub struct Module<T: Trait> for enum Call where origin: T::Origin { }
//     // }

//     #[pallet::type_value]
//     pub fn DefaultValidators<T: Config>() -> Option<Vec<T::AccountId>> {
//         None
//     }

//     #[pallet::storage]
//     #[pallet::getter(fn validators)]
//     pub type Validators<T: Config> =
//         StorageValue<_, Option<Vec<T::AccountId>>, ValueQuery, DefaultValidators<T>>;

//     #[pallet::type_value]
//     pub fn DefaultSessionForValidatorsChange<T: Config>() -> Option<u32> {
//         None
//     }

//     #[pallet::storage]
//     #[pallet::getter(fn session_for_validators_change)]
//     pub type SessionForValidatorsChange<T: Config> =
//         StorageValue<_, Option<u32>, ValueQuery, DefaultSessionForValidatorsChange<T>>;
// }

pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
    let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
    let new_storage_version = crate::STORAGE_VERSION;

    if on_chain_storage_version == StorageVersion::default() && new_storage_version == 1 {
        log::info!(
        target: "pallet_aleph",
        "Running migration from STORAGE_VERSION 0 to 1",
        );

        // TODO : migration logic
        sp_io::storage::clear(&storage_key("Aleph", "Validators"));
        sp_io::storage::clear(&storage_key("Aleph", "SessionForValidatorsChange"));

        // TODO : store new struct
        // TODO : read from current storage values
        // crate:

        // ValidatorsChange::<T>::put(ValidatorsChangeStorageItem::<T> {
        //     validators: Vec::default(),
        //     session_for_validators_change: u32::default(),
        // });

        // store new version
        StorageVersion::new(1).put::<P>();

        // TODO calculate and return migration weights
        // T::DbWeight::get().reads_writes(count as Weight + 1, count as Weight + 1)
        0
    } else {
        log::warn!(
            target: "pallet_aleph",
            "Do not know which migration to apply because on-chain storage version is {:?} and the version declared in the aleph pallet is {:?}",
            on_chain_storage_version,
            new_storage_version
        );
        T::DbWeight::get().reads(1)
    }
}

fn storage_key(module: &str, version: &str) -> [u8; 32] {
    let pallet_name = sp_io::hashing::twox_128(module.as_bytes());
    let postfix = sp_io::hashing::twox_128(version.as_bytes());

    let mut final_key = [0u8; 32];
    final_key[..16].copy_from_slice(&pallet_name);
    final_key[16..].copy_from_slice(&postfix);

    final_key
}
