use crate::Config;
use frame_support::{
    generate_storage_alias, log,
    traits::{Get, PalletInfoAccess, StorageVersion},
    weights::Weight,
};
use sp_std::vec::Vec;

generate_storage_alias!(
    Elections, Members<T: Config> => Value<Vec<T::AccountId>>
);
generate_storage_alias!(
    Elections, MembersPerSession => Value<u32>
);
generate_storage_alias!(
    Elections, ReservedMembers<T: Config> => Value<Vec<T::AccountId>>
);
generate_storage_alias!(
    Elections, ErasReserved<T: Config> => Value<Vec<T::AccountId>>
);

pub fn migrate<T: Config, P: PalletInfoAccess>() -> Weight {
    log::info!(target: "pallet_elections", "Running migration from STORAGE_VERSION 0 to 1");
    let members = Members::<T>::get().expect("");

    let members_per_session = members.len();
    let mut write_count = 0;

    match MembersPerSession::translate(|old| match old {
        Some(Some(x)) => Some(x),
        _ => Some(members_per_session as u32),
    }) {
        Ok(_) => {
            write_count += 1;
        }
        Err(why) => {
            log::error!(target: "pallet_elections", "Something went wrong during the migration of MembersPerSession {:?}", why);
        }
    }

    match ReservedMembers::<T>::translate(|old| match old {
        Some(Some(x)) => Some(x),
        _ => Some(members.clone()),
    }) {
        Ok(_) => {
            write_count += 1;
        }
        Err(why) => {
            log::error!(target: "pallet_elections", "Something went wrong during the migration of ReservedMembers {:?}", why);
        }
    }

    match ErasReserved::<T>::translate(|old| match old {
        Some(Some(x)) => Some(x),
        _ => Some(members),
    }) {
        Ok(_) => {
            write_count += 1;
        }
        Err(why) => {
            log::error!(target: "pallet_elections", "Something went wrong during the migration of ErasReserved {:?}", why);
        }
    }

    StorageVersion::new(1).put::<P>();
    write_count += 1;

    T::DbWeight::get().reads(3) + T::DbWeight::get().writes(write_count)
}
