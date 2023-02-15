use codec::{Decode, Encode};
use frame_election_provider_support::Weight;
use frame_support::{
    log,
    pallet_prelude::{StorageVersion, TypeInfo},
    traits::{OnRuntimeUpgrade, PalletInfoAccess},
};
use primitives::CommitteeSeats;
use sp_core::Get;
#[cfg(feature = "try-runtime")]
use {frame_support::ensure, pallets_support::ensure_storage_version, sp_std::vec::Vec};

use crate::{CommitteeSize, Config, NextEraCommitteeSize};

// V3 CommitteeSeats
#[derive(Decode, Encode, TypeInfo, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitteeSeatsV3 {
    /// Size of reserved validators in a session
    pub reserved_seats: u32,
    /// Size of non reserved validators in a session
    pub non_reserved_seats: u32,
}

/// Migration add field for `CommitteeSize` and `NextEraCommitteeSize` `finality_committee_non_reserved_seats` to
/// `CommitteeSeats`.
pub struct Migration<T, P>(sp_std::marker::PhantomData<(T, P)>);

impl<T: Config, P: PalletInfoAccess> OnRuntimeUpgrade for Migration<T, P> {
    fn on_runtime_upgrade() -> Weight {
        log::info!(target: "pallet_elections", "Running migration from STORAGE_VERSION 3 to 4 for pallet elections");

        let reads = 2;
        let mut writes = 1;

        if CommitteeSize::<T>::translate::<CommitteeSeatsV3, _>(|old| {
            if let Some(CommitteeSeatsV3 {
                reserved_seats,
                non_reserved_seats,
            }) = old
            {
                Some(CommitteeSeats {
                    reserved_seats,
                    non_reserved_seats,
                    non_reserved_finality_seats: non_reserved_seats,
                })
            } else {
                None
            }
        }).is_ok() {
            writes += 1;
        } else {
            log::error!(target: "pallet_elections", "Could not migrate CommitteeSize");
        }

        if NextEraCommitteeSize::<T>::translate::<CommitteeSeatsV3, _>(|old| {
            if let Some(CommitteeSeatsV3 {
                reserved_seats,
                non_reserved_seats,
            }) = old
            {
                Some(CommitteeSeats {
                    reserved_seats,
                    non_reserved_seats,
                    non_reserved_finality_seats: non_reserved_seats,
                })
            } else {
                None
            }
        }).is_ok() {
            writes += 1;
        } else {
            log::error!(target: "pallet_elections", "Could not migrate NextCommitteeSize");
        }

        StorageVersion::new(4).put::<P>();
        T::DbWeight::get().reads(reads) + T::DbWeight::get().writes(writes)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        ensure_storage_version::<P>(3)?;

        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
        ensure_storage_version::<P>(4)?;

        let committee_seats = CommitteeSize::<T>::get();
        ensure!(
            committee_seats.non_reserved_finality_seats == committee_seats.non_reserved_seats,
            "non_reserved_finality_seats should be equal to non_reserved_seats"
        );
        let committee_seats = NextEraCommitteeSize::<T>::get();
        ensure!(
            committee_seats.non_reserved_finality_seats == committee_seats.non_reserved_seats,
            "non_reserved_finality_seats should be equal to non_reserved_seats"
        );

        Ok(())
    }
}
