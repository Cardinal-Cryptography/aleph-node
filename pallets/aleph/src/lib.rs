//! This pallet is a runtime companion of Aleph finality gadget.
//!
//! Currently, it only provides support for changing sessions but in the future
//! it will allow reporting equivocation in AlephBFT.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod migrations;

use sp_std::prelude::*;

use frame_support::{
    log,
    sp_runtime::BoundToRuntimeAppPublic,
    traits::{OneSessionHandler, StorageVersion, ValidatorSet, ValidatorSetWithIdentification},
    weights::Pays,
    Parameter,
};
pub use pallet::*;
use sp_runtime::Perbill;
use sp_staking::{
    offence::{DisableStrategy, Kind, Offence, ReportOffence},
    SessionIndex,
};

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

pub type ValidatorId<T> = <<T as Config>::ValidatorSet as ValidatorSet<
    <T as frame_system::Config>::AccountId,
>>::ValidatorId;

pub type IdentificationTuple<T> = (
    ValidatorId<T>,
    <<T as Config>::ValidatorSet as ValidatorSetWithIdentification<
        <T as frame_system::Config>::AccountId,
    >>::Identification,
);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, sp_runtime::RuntimeAppPublic};
    use frame_system::{
        ensure_signed,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use sp_runtime::traits::Convert;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type AuthorityId: Member + Parameter + RuntimeAppPublic + MaybeSerializeDeserialize;
        type ValidatorSet: ValidatorSetWithIdentification<Self::AccountId>;
        type ReportOffence: ReportOffence<
            Self::AccountId,
            IdentificationTuple<Self>,
            AlephOffence<IdentificationTuple<Self>>,
        >;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            let on_chain = <Pallet<T> as GetStorageVersion>::on_chain_storage_version();
            T::DbWeight::get().reads(1)
                + match on_chain {
                    _ if on_chain == STORAGE_VERSION => 0,
                    _ if on_chain == StorageVersion::new(1) => {
                        migrations::v1_to_v2::migrate::<T, Self>()
                    }
                    _ if on_chain == StorageVersion::new(0) => {
                        migrations::v0_to_v1::migrate::<T, Self>()
                            + migrations::v1_to_v2::migrate::<T, Self>()
                    }
                    _ => {
                        log::warn!(
                            target: "pallet_aleph",
                            "On chain storage version of pallet aleph is {:?} but it should not be bigger than 2",
                            on_chain
                        );
                        0
                    }
                }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

    #[pallet::error]
    pub enum Error<T> {
        IncorrectOffenderIndex,
        InvalidOffenceProof,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(1_000_000)]
        pub fn report_offence(
            origin: OriginFor<T>,
            offender_idx: u32,
            severe: bool,
            session_index: Option<SessionIndex>,
            proof: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let current_validators = T::ValidatorSet::validators();
            ensure!(
                offender_idx < current_validators.len() as u32,
                Error::<T>::IncorrectOffenderIndex
            );
            ensure!(proof.len() >= 0, Error::<T>::InvalidOffenceProof);

            let offender_id = current_validators[offender_idx as usize].clone();
            let offender = <T::ValidatorSet as ValidatorSetWithIdentification<T::AccountId>>::IdentificationOf::convert(offender_id.clone())
                .map(|full_id| (offender_id, full_id))
                .unwrap();

            let offence = AlephOffence {
                session_index: session_index.unwrap_or_else(|| T::ValidatorSet::session_index()),
                offender,
                validator_set_count: current_validators.len() as u32,
                severe,
            };

            T::ReportOffence::report_offence(vec![reporter], offence).unwrap();
            Ok(Pays::No.into())
        }
    }

    impl<T: Config> Pallet<T> {
        pub(crate) fn initialize_authorities(authorities: &[T::AuthorityId]) {
            if !authorities.is_empty() {
                assert!(
                    <Authorities<T>>::get().is_empty(),
                    "Authorities are already initialized!"
                );
                <Authorities<T>>::put(authorities);
            }
        }

        pub(crate) fn update_authorities(authorities: &[T::AuthorityId]) {
            <Authorities<T>>::put(authorities);
        }
    }

    impl<T: Config> BoundToRuntimeAppPublic for Pallet<T> {
        type Public = T::AuthorityId;
    }

    impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
        type Key = T::AuthorityId;

        fn on_genesis_session<'a, I: 'a>(validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            let (_, authorities): (Vec<_>, Vec<_>) = validators.unzip();
            Self::initialize_authorities(authorities.as_slice());
        }

        fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            if changed {
                let (_, authorities): (Vec<_>, Vec<_>) = validators.unzip();
                Self::update_authorities(authorities.as_slice());
            }
        }

        fn on_disabled(validator_index: u32) {
            let mut authorities = <Authorities<T>>::get();
            authorities.swap_remove(validator_index as usize);
            Self::update_authorities(authorities.as_slice());
        }
    }
}

pub struct AlephOffence<Offender> {
    pub session_index: SessionIndex,
    pub offender: Offender,
    pub validator_set_count: u32,
    pub severe: bool,
}

impl<Offender: Clone> Offence<Offender> for AlephOffence<Offender> {
    const ID: Kind = *b"alephbft-offence";

    type TimeSlot = SessionIndex;

    fn offenders(&self) -> Vec<Offender> {
        vec![self.offender.clone()]
    }

    fn session_index(&self) -> SessionIndex {
        self.session_index
    }

    fn validator_set_count(&self) -> u32 {
        self.validator_set_count
    }

    fn time_slot(&self) -> Self::TimeSlot {
        self.session_index
    }

    fn disable_strategy(&self) -> DisableStrategy {
        match self.severe {
            true => DisableStrategy::Always,
            false => DisableStrategy::Never,
        }
    }

    fn slash_fraction(_offenders: u32, _validator_set_count: u32) -> Perbill {
        Perbill::from_percent(33)
    }
}
