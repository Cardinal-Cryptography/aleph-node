#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use frame_support::Parameter;
use sp_std::prelude::*;

use frame_support::{sp_runtime::BoundToRuntimeAppPublic, traits::OneSessionHandler};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::{traits::OpaqueKeys, RuntimeAppPublic},
        sp_std,
    };
    use frame_system::pallet_prelude::*;
    use pallet_session::{Pallet as Session, SessionManager};
    use primitives::{
        ApiError as AlephApiError, DEFAULT_MILLISECS_PER_BLOCK, DEFAULT_SESSION_PERIOD,
    };

    #[pallet::type_value]
    pub fn DefaultValidatorsList<T: Config>() -> Option<Vec<T::AccountId>> {
        None
    }

    #[pallet::storage]
    #[pallet::getter(fn validators_list)]
    pub type ValidatorsList<T: Config> =
        StorageValue<_, Option<Vec<T::AccountId>>, ValueQuery, DefaultValidatorsList<T>>;

    #[pallet::type_value]
    pub fn DefaultValidatorsListSession<T: Config>() -> Option<u32> {
        None
    }

    #[pallet::storage]
    #[pallet::getter(fn validators_list_session)]
    pub type ValidatorsListSession<T: Config> =
        StorageValue<_, Option<u32>, ValueQuery, DefaultValidatorsListSession<T>>;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_session::Config {
        type AuthorityId: Member
            + Parameter
            + RuntimeAppPublic
            + Default
            + MaybeSerializeDeserialize;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ChangeValidators(Option<Vec<T::AccountId>>, u32),
    }

    pub struct AlephSessionManager<T>(sp_std::marker::PhantomData<T>);

    #[pallet::pallet]
    pub struct Pallet<T>(sp_std::marker::PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn change_validators(
            origin: OriginFor<T>,
            validators_list: Option<Vec<T::AccountId>>,
            validators_list_session: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ValidatorsList::<T>::put(validators_list.clone());
            ValidatorsListSession::<T>::put(Some(validators_list_session));
            Self::deposit_event(Event::ChangeValidators(
                validators_list,
                validators_list_session,
            ));
            Ok(())
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn DefaultForSessionPeriod() -> u32 {
        DEFAULT_SESSION_PERIOD
    }

    #[pallet::storage]
    #[pallet::getter(fn session_period)]
    pub(super) type SessionPeriod<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultForSessionPeriod>;

    #[pallet::type_value]
    pub(super) fn DefaultForMillisecsPerBlock() -> u64 {
        DEFAULT_MILLISECS_PER_BLOCK
    }

    #[pallet::storage]
    #[pallet::getter(fn millisecs_per_block)]
    pub(super) type MillisecsPerBlock<T: Config> =
        StorageValue<_, u64, ValueQuery, DefaultForMillisecsPerBlock>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub authorities: Vec<T::AuthorityId>,
        pub session_period: u32,
        pub millisecs_per_block: u64,
        pub validators_list: Option<Vec<T::AccountId>>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                authorities: Vec::new(),
                session_period: DEFAULT_SESSION_PERIOD,
                millisecs_per_block: DEFAULT_MILLISECS_PER_BLOCK,
                validators_list: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <SessionPeriod<T>>::put(&self.session_period);
            <MillisecsPerBlock<T>>::put(&self.millisecs_per_block);
            <ValidatorsList<T>>::put(&self.validators_list);
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

        pub fn next_session_authorities() -> Result<Vec<T::AuthorityId>, AlephApiError> {
            Session::<T>::queued_keys()
                .iter()
                .map(|(_, key)| key.get(T::AuthorityId::ID).ok_or(AlephApiError::DecodeKey))
                .collect::<Result<Vec<T::AuthorityId>, AlephApiError>>()
        }
    }

    impl<T: Config> SessionManager<T::AccountId> for AlephSessionManager<T> {
        fn new_session(session: u32) -> Option<Vec<T::AccountId>> {
            if let Some(validators_list_session) = Pallet::<T>::validators_list_session() {
                if validators_list_session <= session {
                    let result = Pallet::<T>::validators_list();
                    ValidatorsList::<T>::put(None::<Vec<T::AccountId>>);
                    ValidatorsListSession::<T>::put(None::<u32>);
                    return result;
                }
            }
            None
        }

        fn start_session(_: u32) {}

        fn end_session(_: u32) {}
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
            let authorities = validators.map(|(_, key)| key).collect::<Vec<_>>();
            Self::initialize_authorities(authorities.as_slice());
        }

        fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            let authorities = validators.map(|(_, key)| key).collect::<Vec<_>>();
            Self::update_authorities(authorities.as_slice());
        }

        fn on_disabled(_validator_index: usize) {}
    }
}
