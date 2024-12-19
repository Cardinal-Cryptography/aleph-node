use frame_support::{
    pallet_prelude::{PalletInfoAccess, StorageVersion, ValueQuery, Weight},
    storage_alias,
    traits::OnRuntimeUpgrade,
};
use log::info;
use primitives::{ProductionBanConfig, SessionValidators};

use crate::{CurrentAndNextSessionValidators, CurrentAndNextSessionValidatorsStorage};

#[derive(Decode, Encode, TypeInfo, Clone, Serialize, Deserialize)]
pub struct SessionValidatorsLegacy<T> {
    pub committee: Vec<T>,
    pub non_committee: Vec<T>,
}

impl<T> Default for SessionValidatorsLegacy<T> {
    fn default() -> Self {
        Self {
            committee: Vec::new(),
            non_committee: Vec::new(),
        }
    }
}

#[derive(Decode, Encode, TypeInfo)]
pub struct CurrentAndNextSessionValidatorsLegacy<T> {
    pub next: SessionValidatorsLegacy<T>,
    pub current: SessionValidatorsLegacy<T>,
}

impl<T> Default for CurrentAndNextSessionValidators<T> {
    fn default() -> Self {
        Self {
            next: Default::default(),
            current: Default::default(),
        }
    }
}

#[storage_alias]
type BanConfig<T: Config> = StorageValue<Pallet<T>, ProductionBanConfig, ValueQuery>;

/// In order to run both pre- and post- checks around every migration, we entangle methods of
/// `OnRuntimeUpgrade` into the desired flow and expose it with `migrate` method.
///
/// This way, `try-runtime` no longer triggers checks. We do it by hand.
pub trait StorageMigration: OnRuntimeUpgrade {
    #[allow(clippy::let_and_return)]
    fn migrate() -> Weight {
        #[cfg(feature = "try-runtime")]
        let state = Self::pre_upgrade().expect("Pre upgrade should succeed");

        let weight = Self::on_runtime_upgrade();

        CurrentAndNextSessionValidatorsStorage::<T>::translate::<(
            CurrentAndNextSessionValidatorsLegacy<T>,
            _,
        )>(|current_validators_legacy| {
            info!(target: LOG_TARGET, "  Migration ");
            let current_validators = SessionValidators {
                producers: current_validators_legacy.current.committee,
                finalizers: Vec::new(),
                non_committee: current_validators_legacy.current.noncommittee,
            };
            let next_validators = SessionValidators {
                producers: current_validators_legacy.next.committee,
                finalizers: Vec::new(),
                non_committee: current_validators_legacy.next.noncommittee,
            };

            Some(CurrentAndNextSessionValidators {
                current: current_validators,
                next: next_validators,
            })
        });

        let ban_config = BanConfig::<T>::get();
        ProductionBanConfig::<T>::put(ban_config);
        BanConfig::<T>::kill();

        #[cfg(feature = "try-runtime")]
        Self::post_upgrade(state).expect("Post upgrade should succeed");

        weight
    }
}

impl<T: OnRuntimeUpgrade> StorageMigration for T {}

/// Ensure that the current pallet storage version matches `version`.
pub fn ensure_storage_version<P: PalletInfoAccess>(version: u16) -> Result<(), &'static str> {
    if StorageVersion::get::<P>() == StorageVersion::new(version) {
        Ok(())
    } else {
        Err("Bad storage version")
    }
}
