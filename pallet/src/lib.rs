#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::Parameter;
use sp_std::prelude::*;

use frame_support::{sp_runtime::BoundToRuntimeAppPublic, traits::OneSessionHandler};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::{DigestItem, RuntimeAppPublic},
        sp_std,
    };
    use frame_system::pallet_prelude::*;
    use primitives::{LogChange, ALEPH_ENGINE_ID};

    #[derive(Encode, Decode)]
    pub struct SessionChange<T>
    where
        T: Config,
    {
        /// The block number the session was created.
        pub created_at: T::BlockNumber,
        pub session_id: u64,
        pub changed: bool,
        pub next_authorities: Vec<T::AuthorityId>,
    }

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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(block_number: T::BlockNumber) {
            if let Some(session_info) = <SessionInfo<T>>::get() {
                if session_info.changed && session_info.created_at == block_number {
                    Self::update_authorities(session_info.next_authorities.as_slice());
                    Self::deposit_log(LogChange::NewAuthorities(
                        session_info.next_authorities,
                        session_info.session_id,
                    ));
                }
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn session_info)]
    pub(super) type SessionInfo<T> = StorageValue<_, SessionChange<T>, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub authorities: Vec<T::AuthorityId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                authorities: Vec::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {}
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

            <SessionInfo<T>>::put(SessionChange {
                session_id: 0,
                changed: true,
                created_at: <frame_system::Pallet<T>>::block_number(),
                next_authorities: authorities.to_vec(),
            })
        }

        fn update_authorities(authorities: &[T::AuthorityId]) {
            <Authorities<T>>::put(authorities);
        }

        fn new_session(changed: bool, authorities: Vec<T::AuthorityId>) {
            if let Some(old_session) = <SessionInfo<T>>::get() {
                let current_block = <frame_system::Pallet<T>>::block_number();

                <SessionInfo<T>>::put(SessionChange {
                    session_id: old_session.session_id + 1,
                    changed,
                    created_at: current_block,
                    next_authorities: authorities,
                });
            }
        }

        /// Deposit one of this module's logs.
        fn deposit_log(change: LogChange<T::AuthorityId>) {
            let log: DigestItem<T::Hash> = DigestItem::Consensus(ALEPH_ENGINE_ID, change.encode());
            <frame_system::Pallet<T>>::deposit_log(log);
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
            let authorities = validators.map(|(_, key)| key).collect::<Vec<_>>();
            Self::initialize_authorities(authorities.as_slice());
        }

        fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
        where
            I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
            T::AccountId: 'a,
        {
            let authorities = validators.map(|(_, key)| key).collect::<Vec<_>>();
            Self::new_session(changed, authorities)
        }

        fn on_disabled(_validator_index: usize) {}
    }
}
