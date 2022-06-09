use crate::{
    CommitteeSize, Config, CurrentEraValidators, NextEraNonReservedValidators,
    NextEraReservedValidators,
};
use frame_support::{
    log, storage_alias,
    traits::{Get, PalletInfoAccess, StorageVersion},
    weights::Weight,
};
use sp_std::vec::Vec;

#[storage_alias]
pub type MembersPerSession = StorageValue<Elections, u32>;
#[storage_alias]
type ReservedMembers<T> = StorageValue<Elections, Vec<<T as frame_system::Config>::AccountId>>;
#[storage_alias]
type NonReservedMembers<T> = StorageValue<Elections, Vec<<T as frame_system::Config>::AccountId>>;
#[storage_alias]
type ErasMembers<T> = StorageValue<
    Elections,
    (
        Vec<<T as frame_system::Config>::AccountId>,
        Vec<<T as frame_system::Config>::AccountId>,
    ),
>;

/// The assumptions made by this migration:
///
///
pub fn migrate<T: Config, P: PalletInfoAccess>() -> Weight {
    log::info!(target: "pallet_elections", "Running migration from STORAGE_VERSION 0 to 1 for pallet elections");

    let writes = 5;
    let reads = 4;

    let mps = MembersPerSession::get().expect("");
    let reserved = ReservedMembers::<T>::get().expect("");
    let non_reserved = NonReservedMembers::<T>::get().expect("");
    let eras_members = ErasMembers::<T>::get().expect("");

    CommitteeSize::<T>::put(mps);
    NextEraReservedValidators::<T>::put(reserved);
    NextEraNonReservedValidators::<T>::put(non_reserved);
    CurrentEraValidators::<T>::put(eras_members);

    MembersPerSession::kill();
    ReservedMembers::<T>::kill();
    NonReservedMembers::<T>::kill();
    ErasMembers::<T>::kill();

    StorageVersion::new(2).put::<P>();
    T::DbWeight::get().reads(reads) + T::DbWeight::get().writes(writes)
}
