#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

/// We store verification keys under short identifiers.
pub type VerificationKeyIdentifier = [u8; 4];

#[frame_support::pallet]
pub mod pallet {
    use ark_ec::PairingEngine;
    use ark_groth16::{Groth16, Proof, VerifyingKey};
    use ark_serialize::CanonicalDeserialize;
    use ark_snark::SNARK;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;
    use sp_std::prelude::Vec;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Field: PairingEngine;

        #[pallet::constant]
        type MaximumVerificationKeyLength: Get<u32>;
    }

    #[pallet::error]
    pub enum Error<T> {
        /// This verification key identifier is already taken.
        IdentifierAlreadyInUse,
        /// There is no verification key available under this identifier.
        UnknownVerificationKeyIdentifier,
        /// Provided verification key is longer than `MaximumVerificationKeyLength` limit.
        VerificationKeyTooLong,
        /// Couldn't deserialize proof.
        DeserializingProofFailed,
        /// Couldn't deserialize verification key from storage.
        DeserializingVerificationKeyFailed,
        /// Verification procedure has failed. Proof still can be correct.
        VerificationFailed,
        /// Proof has been found as incorrect.
        IncorrectProof,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Verification key has been successfully stored.
        VerificationKeyStored,
        /// Proof has been successfully verified.
        VerificationSucceeded,
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type VerificationKeys<T: Config> = StorageMap<
        _,
        Twox64Concat,
        VerificationKeyIdentifier,
        BoundedVec<u8, T::MaximumVerificationKeyLength>,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(41)]
        pub fn store_key(
            _origin: OriginFor<T>,
            identifier: VerificationKeyIdentifier,
            key: Vec<u8>,
        ) -> DispatchResult {
            ensure!(
                !VerificationKeys::<T>::contains_key(identifier.clone()),
                Error::<T>::IdentifierAlreadyInUse
            );
            ensure!(
                key.len() <= T::MaximumVerificationKeyLength::get() as usize,
                Error::<T>::VerificationKeyTooLong
            );

            VerificationKeys::<T>::insert(
                identifier,
                BoundedVec::try_from(key).unwrap(), // must succeed since we've just check length
            );

            Self::deposit_event(Event::VerificationKeyStored);
            Ok(())
        }

        #[pallet::weight(4141)]
        pub fn verify(
            _origin: OriginFor<T>,
            verification_key_identifier: VerificationKeyIdentifier,
            proof: Vec<u8>,
            _public_input: (),
        ) -> DispatchResult {
            let proof = Proof::<T::Field>::deserialize(&*proof)
                .map_err(|_| Error::<T>::DeserializingProofFailed)?;

            let verification_key = VerificationKeys::<T>::get(verification_key_identifier)
                .ok_or(Error::<T>::UnknownVerificationKeyIdentifier)?;
            let verification_key = VerifyingKey::<T::Field>::deserialize(&**verification_key)
                .map_err(|_| Error::<T>::DeserializingVerificationKeyFailed)?;

            let public_input = Vec::new();

            let valid_proof = Groth16::verify(&verification_key, &public_input, &proof)
                .map_err(|_| Error::<T>::VerificationFailed)?;

            ensure!(valid_proof, Error::<T>::IncorrectProof);

            Self::deposit_event(Event::VerificationSucceeded);
            Ok(())
        }
    }
}
