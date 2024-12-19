use frame_support::{
    pallet_prelude::{StorageVersion, ValueQuery, Weight},
    storage_alias,
    traits::OnRuntimeUpgrade,
};
use log::info;
use parity_scale_codec::Decode;
use primitives::{ProductionBanConfig as ProductionBanConfigStruct, SessionValidators};

use crate::{CurrentAndNextSessionValidators, CurrentAndNextSessionValidatorsStorage};

pub mod v1 {
    use frame_support::traits::Get;
    use parity_scale_codec::Encode;
    use primitives::KEY_TYPE;
    use sp_runtime::traits::OpaqueKeys;

    use super::*;
    use crate::{Config, Pallet, ProductionBanConfig, LOG_TARGET};

    #[derive(Decode)]
    pub struct SessionValidatorsLegacy<T> {
        pub committee: Vec<T>,
        pub non_committee: Vec<T>,
    }

    #[derive(Decode)]
    pub struct CurrentAndNextSessionValidatorsLegacy<T> {
        pub next: SessionValidatorsLegacy<T>,
        pub current: SessionValidatorsLegacy<T>,
    }

    #[storage_alias]
    type BanConfig<T: Config> = StorageValue<Pallet<T>, ProductionBanConfigStruct, ValueQuery>;

    pub struct Migration<T>(sp_std::marker::PhantomData<T>);

    impl<T: Config + pallet_aleph::Config + pallet_session::Config> OnRuntimeUpgrade for Migration<T> {
        fn on_runtime_upgrade() -> Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(3) {
                log::info!(
                    target: LOG_TARGET,
                    "Skipping migrations from STORAGE_VERSION 0 to 1 for pallet committee-management."
                );
                return T::DbWeight::get().reads(1);
            };

            let reads = 5; // StorageVersion, CurrentAndNextSessionValidatorsStorage, Authorities, NextFinalityCommittee,  BanConfig
            let mut writes = 2; // StorageVersion, ProductionBanConfig
            info!(target: LOG_TARGET, "Running migration from STORAGE_VERSION 0 to 1 for pallet committee-management.");

            let res = CurrentAndNextSessionValidatorsStorage::<T>::translate::<
                CurrentAndNextSessionValidatorsLegacy<T::AccountId>,
                _,
            >(|current_validators_legacy| {
                let current_validators_legacy =
                    current_validators_legacy.expect("This storage exists");

                let aleph_authorities = pallet_aleph::Authorities::<T>::get();
                let mut finalizers = Vec::new();
                for (v, keys) in pallet_session::QueuedKeys::<T>::get().into_iter() {
                    let aleph_key = keys
                        .get::<<T as pallet_aleph::Config>::AuthorityId>(KEY_TYPE)
                        .unwrap();

                    if aleph_authorities.iter().any(|aa| aa.eq(&aleph_key)) {
                        let v = T::AccountId::decode(&mut v.encode().as_ref()).unwrap();
                        finalizers.push(v);
                    }
                }

                let current_validators = SessionValidators {
                    producers: current_validators_legacy.current.committee,
                    finalizers,
                    non_committee: current_validators_legacy.current.non_committee,
                };
                let next_validators = SessionValidators {
                    producers: current_validators_legacy.next.committee,
                    finalizers: pallet_aleph::NextFinalityCommittee::<T>::get(),
                    non_committee: current_validators_legacy.next.non_committee,
                };

                Some(CurrentAndNextSessionValidators {
                    current: current_validators,
                    next: next_validators,
                })
            });
            if res.is_ok() {
                writes += 1;
            } else {
                log::error!(target: LOG_TARGET, "Could not migrate CurrentAndNextSessionValidatorsStorage.");
            };

            let ban_config = BanConfig::<T>::get();
            ProductionBanConfig::<T>::put(ban_config);
            BanConfig::<T>::kill();

            StorageVersion::new(1).put::<Pallet<T>>();
            T::DbWeight::get().reads(reads) + T::DbWeight::get().writes(writes)
        }
    }
}
