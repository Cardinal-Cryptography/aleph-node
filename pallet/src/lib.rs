#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::Parameter;
use sp_std::prelude::*;

use frame_support::traits::OneSessionHandler;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, sp_runtime::RuntimeAppPublic, sp_std};
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type AuthorityId: Member
            + Parameter
            + RuntimeAppPublic
            + Default
            + MaybeSerializeDeserialize;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(sp_std::marker::PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub(super) type Validators<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub authorities: Vec<T::AuthorityId>,
        pub validators: Vec<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                authorities: Vec::new(),
                validators: Vec::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            Pallet::<T>::initialize_authorities(&self.authorities);
            Pallet::<T>::initialize_validators(&self.validators);
        }
    }
}

impl<T: Config> Pallet<T> {
    fn initialize_authorities(authorities: &[T::AuthorityId]) {
        if !authorities.is_empty() {
            assert!(
                <Authorities<T>>::get().is_empty(),
                "Authorities are already initialized!"
            );
            <Authorities<T>>::put(authorities);
        }
    }
}

impl<T: Config> Pallet<T> {
    fn initialize_validators(validators: &[T::AccountId]) {
        if !validators.is_empty() {
            assert!(
                <Validators<T>>::get().is_empty(),
                "Validators are already initialized!"
            );
            <Validators<T>>::put(validators);
        }
    }
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
    type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
    type Key = T::AuthorityId;

    fn on_genesis_session<'a, I: 'a>(validators: I)
    where
        I: Iterator<Item = (&'a T::AccountId, Self::Key)>,
        T::AccountId: 'a,
    {
        let authorities: Vec<_> = validators.map(|(_, aleph_id)| aleph_id).collect();
        <Authorities<T>>::put(authorities);
    }

    fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
    where
        I: Iterator<Item = (&'a T::AccountId, Self::Key)>,
        T::AccountId: 'a,
    {
        if changed {
            let authorities: Vec<_> = validators.map(|(_, aleph_id)| aleph_id).collect();
            <Authorities<T>>::put(authorities);
        }
    }

    fn on_disabled(_validator_index: usize) {}
}
