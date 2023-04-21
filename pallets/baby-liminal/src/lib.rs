#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod systems;
#[cfg(test)]
mod tests;
mod weights;

use frame_support::{
    fail,
    pallet_prelude::StorageVersion,
    traits::{Currency, ReservableCurrency},
};
use frame_system::ensure_signed;
pub use pallet::*;
pub use systems::VerificationError;
pub use weights::{AlephWeight, WeightInfo};

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

/// We store proving and verification keys under short identifiers.
pub type KeyPairIdentifier = [u8; 8];
pub type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {

    use ark_serialize::CanonicalDeserialize;
    use frame_support::{
        dispatch::PostDispatchInfo, log, pallet_prelude::*, sp_runtime::DispatchErrorWithPostInfo,
    };
    use frame_system::pallet_prelude::OriginFor;
    use sp_std::{
        cmp::Ordering::{Equal, Greater, Less},
        prelude::Vec,
    };

    use super::*;
    use crate::systems::{Groth16, VerificationError, VerifyingSystem};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Limits how many bytes the proving key can have.
        ///
        /// Proving keys are stored, therefore this is separated from the limits on proof or
        /// public input.
        #[pallet::constant]
        type MaximumProvingKeyLength: Get<u32>;

        /// Limits how many bytes verification key can have.
        ///
        /// Verification keys are stored, therefore this is separated from the limits on proof or
        /// public input.
        #[pallet::constant]
        type MaximumVerificationKeyLength: Get<u32>;

        /// Limits how many bytes proof or public input can have.
        #[pallet::constant]
        type MaximumDataLength: Get<u32>;

        /// Deposit amount for storing a proving/verification key pair
        ///
        /// Will get locked and returned upon deleting the key pair by the owner
        #[pallet::constant]
        type KeyPairDepositPerByte: Get<BalanceOf<Self>>;
    }

    #[pallet::error]
    #[derive(Clone, Eq, PartialEq)]
    pub enum Error<T> {
        /// This proving/verification key pair identifier is already taken.
        IdentifierAlreadyInUse,
        /// There is no proving/verification key pair available under this identifier.
        UnknownKeyPairIdentifier,
        /// Provided proving key is longer than `MaximumProvingKeyLength` limit.
        ProvingKeyTooLong,
        /// Provided verification key is longer than `MaximumVerificationKeyLength` limit.
        VerificationKeyTooLong,
        /// Either proof or public input is longer than `MaximumDataLength` limit.
        DataTooLong,
        /// Couldn't deserialize proof.
        DeserializingProofFailed,
        /// Couldn't deserialize public input.
        DeserializingPublicInputFailed,
        /// Couldn't deserialize verification key from storage.
        DeserializingVerificationKeyFailed,
        /// Verification procedure has failed. Proof still can be correct.
        VerificationFailed(VerificationError),
        /// Proof has been found as incorrect.
        IncorrectProof,

        /// Unsigned request
        BadOrigin,
        /// User has insufficient funds to lock the deposit for storing verification key
        CannotAffordDeposit,
        /// Caller is not the owner of the key
        NotOwner,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Proving and verification keys have been successfully stored.
        ///
        /// \[ account_id, identifier \]
        KeyPairStored(T::AccountId, KeyPairIdentifier),

        /// Proving and verification keys have been successfully deleted.
        ///
        /// \[ identifier \]
        KeyPairDeleted(T::AccountId, KeyPairIdentifier),

        /// Proving and verification keys have been successfully overwritten.
        ///
        /// \[ identifier \]
        KeyPairOverwritten(KeyPairIdentifier),

        /// Proof has been successfully verified.
        VerificationSucceeded,
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn get_proving_key)]
    pub type ProvingKeys<T: Config> =
        StorageMap<_, Twox64Concat, KeyPairIdentifier, BoundedVec<u8, T::MaximumProvingKeyLength>>;

    #[pallet::storage]
    #[pallet::getter(fn get_verification_key)]
    pub type VerificationKeys<T: Config> = StorageMap<
        _,
        Twox64Concat,
        KeyPairIdentifier,
        BoundedVec<u8, T::MaximumVerificationKeyLength>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn get_key_pair_owner)]
    pub type KeyPairOwners<T: Config> =
        StorageMap<_, Twox64Concat, KeyPairIdentifier, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn get_key_pair_deposit)]
    pub type KeyPairDeposits<T: Config> =
        StorageMap<_, Twox64Concat, (T::AccountId, KeyPairIdentifier), BalanceOf<T>>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Stores `proving_key` under `identifier` in `ProvingKeys` map.
        /// Stores `verification_key` under `identifier` in `VerificationKeys` map.
        ///
        /// Fails if:
        /// - `proving_key.len()` is greater than `MaximumProvingKeyLength`, or
        /// - `verification_key.len()` is greater than `MaximumVerificationKeyLength`, or
        /// - `identifier` has been already used
        ///
        /// `proving_key` and `verification_key` can come from any proving system - there are no
        /// checks that verify them, in particular, they both can just contain trash bytes.
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn store_key_pair(
            origin: OriginFor<T>,
            identifier: KeyPairIdentifier,
            proving_key: Vec<u8>,
            verification_key: Vec<u8>,
        ) -> DispatchResult {
            Self::bare_store_key_pair(origin, identifier, proving_key, verification_key)
                .map_err(|e| e.into())
        }

        /// Deletes keys stored under `identifier` in both the `ProvingKeys` and `VerificationKeys`
        /// maps.
        ///
        /// Returns the deposit locked. Can only be called by the key pair owner.
        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn delete_key_pair(
            origin: OriginFor<T>,
            identifier: KeyPairIdentifier,
        ) -> DispatchResult {
            let who = ensure_signed(origin).map_err(|_| Error::<T>::BadOrigin)?;
            let owner =
                KeyPairOwners::<T>::get(identifier).ok_or(Error::<T>::UnknownKeyPairIdentifier)?;

            ensure!(who == owner, Error::<T>::NotOwner);

            let deposit = KeyPairDeposits::<T>::take((&owner, &identifier)).unwrap(); // cannot fail since the key pair has owner and owner must have made a deposit
            T::Currency::unreserve(&owner, deposit);

            ProvingKeys::<T>::remove(identifier);
            VerificationKeys::<T>::remove(identifier);
            Self::deposit_event(Event::KeyPairDeleted(who, identifier));
            Ok(())
        }

        /// Overwrites keys stored under `identifier` in the `ProvingKeys` and `VerificationKeys`
        /// maps with new values `proving_key` and `verification_key`, respectively.
        ///
        /// Fails if `proving_key.len()` is greater than `MaximumProvingKeyLength` or
        /// `verification_key.len()` is greater than `MaximumVerificationKeyLength`.
        /// Can only be called by the original owner of the key.
        /// It will require the caller to lock up additional funds (if the new key pair occupies more storage)
        /// or reimburse the difference if it is shorter in its byte-length.
        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn overwrite_key_pair(
            origin: OriginFor<T>,
            identifier: KeyPairIdentifier,
            proving_key: Vec<u8>,
            verification_key: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin).map_err(|_| Error::<T>::BadOrigin)?;
            let owner = KeyPairOwners::<T>::get(identifier);

            match owner {
                Some(owner) => ensure!(who == owner, Error::<T>::NotOwner),
                None => fail!(Error::<T>::UnknownKeyPairIdentifier),
            };

            ensure!(
                proving_key.len() <= T::MaximumProvingKeyLength::get() as usize,
                Error::<T>::ProvingKeyTooLong
            );
            ensure!(
                verification_key.len() <= T::MaximumVerificationKeyLength::get() as usize,
                Error::<T>::VerificationKeyTooLong
            );

            ProvingKeys::<T>::try_mutate_exists(identifier, |value| -> DispatchResult {
                // should never fail, since length is checked above
                *value = Some(BoundedVec::try_from(proving_key.clone()).unwrap());
                Ok(())
            })?;
            VerificationKeys::<T>::try_mutate_exists(identifier, |value| -> DispatchResult {
                // should never fail, since length is checked above
                *value = Some(BoundedVec::try_from(verification_key.clone()).unwrap());
                Ok(())
            })?;

            KeyPairDeposits::<T>::try_mutate_exists(
                (&who, &identifier),
                |maybe_previous_deposit| -> DispatchResult {
                    let previous_deposit =
                        maybe_previous_deposit.ok_or(Error::<T>::UnknownKeyPairIdentifier)?;

                    let key_pair_len = proving_key.len() + verification_key.len();
                    let deposit =
                        T::KeyPairDepositPerByte::get() * BalanceOf::<T>::from(key_pair_len as u32);

                    match deposit.cmp(&previous_deposit) {
                        Less => {
                            // reimburse the prev - deposit difference
                            // we know that the caller is the owner because we have checked that
                            let difference = previous_deposit - deposit;
                            T::Currency::unreserve(&who, difference);
                            *maybe_previous_deposit = Some(deposit);
                        }
                        Equal => {
                            // do nothing
                        }
                        Greater => {
                            // lock the difference deposit - prev
                            let difference = deposit - previous_deposit;
                            T::Currency::reserve(&who, difference)
                                .map_err(|_| Error::<T>::CannotAffordDeposit)?;
                            *maybe_previous_deposit = Some(deposit);
                        }
                    };

                    Self::deposit_event(Event::KeyPairOverwritten(identifier));
                    Ok(())
                },
            )
        }

        /// Verifies `proof` against `public_input` with a key that has been stored under
        /// `verification_key_identifier`. All is done within Groth16 proving system.
        ///
        /// Fails if:
        /// - there is no verification key under `verification_key_identifier`
        /// - verification key under `verification_key_identifier` cannot be deserialized
        /// (e.g. it has been produced for another proving system)
        /// - `proof` cannot be deserialized (e.g. it has been produced for another proving system)
        /// - `public_input` cannot be deserialized (e.g. it has been produced for another proving
        /// system)
        /// - verifying procedure fails (e.g. incompatible verification key and proof)
        /// - proof is incorrect
        #[pallet::weight(0)]
        #[pallet::call_index(3)]
        pub fn verify(
            _origin: OriginFor<T>,
            verification_key_identifier: KeyPairIdentifier,
            proof: Vec<u8>,
            public_input: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            Self::bare_verify(verification_key_identifier, proof, public_input)
                .map(|_| ().into())
                .map_err(|(error, actual_weight)| DispatchErrorWithPostInfo {
                    post_info: PostDispatchInfo {
                        pays_fee: Pays::Yes,
                        actual_weight,
                    },
                    error: error.into(),
                })
        }
    }

    impl<T: Config> Pallet<T> {
        /// This is the inner logic behind `Self::store_key_pair`, however it is free from account
        /// lookup or other dispatchable-related overhead. Thus, it is more suited to call directly
        /// from runtime, like from a chain extension.
        pub fn bare_store_key_pair(
            origin: OriginFor<T>,
            identifier: KeyPairIdentifier,
            proving_key: Vec<u8>,
            verification_key: Vec<u8>,
        ) -> Result<(), Error<T>> {
            let who = ensure_signed(origin).map_err(|_| Error::<T>::BadOrigin)?;

            ensure!(
                proving_key.len() <= T::MaximumProvingKeyLength::get() as usize,
                Error::<T>::ProvingKeyTooLong
            );
            ensure!(
                verification_key.len() <= T::MaximumVerificationKeyLength::get() as usize,
                Error::<T>::VerificationKeyTooLong
            );

            ensure!(
                !(VerificationKeys::<T>::contains_key(identifier)
                    || ProvingKeys::<T>::contains_key(identifier)),
                Error::<T>::IdentifierAlreadyInUse
            );

            // make a locked deposit that will be returned when the key pair is deleted
            // deposit is calculated per byte of occupied storage
            let key_pair_len = proving_key.len() + verification_key.len();
            let deposit =
                T::KeyPairDepositPerByte::get() * BalanceOf::<T>::from(key_pair_len as u32);
            T::Currency::reserve(&who, deposit).map_err(|_| Error::<T>::CannotAffordDeposit)?;

            ProvingKeys::<T>::insert(
                identifier,
                BoundedVec::try_from(proving_key).unwrap(), // must succeed since we've just check length
            );
            VerificationKeys::<T>::insert(
                identifier,
                BoundedVec::try_from(verification_key).unwrap(), // must succeed since we've just check length
            );

            // will never overwrite anything since we have already checked the ProvingKeys and
            // VerificationKeys maps
            KeyPairOwners::<T>::insert(identifier, &who);
            KeyPairDeposits::<T>::insert((&who, &identifier), deposit);

            Self::deposit_event(Event::KeyPairStored(who, identifier));
            Ok(())
        }

        /// This is the inner logic behind `Self::verify`, however it is free from account lookup
        /// or other dispatchable-related overhead. Thus, it is more suited to call directly from
        /// runtime, like from a chain extension.
        pub fn bare_verify(
            verification_key_identifier: KeyPairIdentifier,
            proof: Vec<u8>,
            public_input: Vec<u8>,
        ) -> Result<(), (Error<T>, Option<Weight>)> {
            Self::_bare_verify::<Groth16>(verification_key_identifier, proof, public_input)
        }

        fn _bare_verify<S: VerifyingSystem>(
            verification_key_identifier: KeyPairIdentifier,
            proof: Vec<u8>,
            public_input: Vec<u8>,
        ) -> Result<(), (Error<T>, Option<Weight>)> {
            let data_length_limit = T::MaximumDataLength::get() as usize;
            let data_length_excess = proof.len().saturating_sub(data_length_limit)
                + public_input.len().saturating_sub(data_length_limit);
            ensure!(
                data_length_excess == 0,
                (
                    Error::<T>::DataTooLong,
                    Some(T::WeightInfo::verify_data_too_long(
                        data_length_excess as u32
                    ))
                )
            );

            let proof_len = proof.len() as u32;
            let proof: S::Proof = CanonicalDeserialize::deserialize(&*proof).map_err(|e| {
                log::error!("Deserializing proof failed: {:?}", e);
                (
                    Error::<T>::DeserializingProofFailed,
                    Some(T::WeightInfo::verify_data_deserializing_fails(proof_len)),
                )
            })?;

            let public_input: Vec<S::CircuitField> =
                CanonicalDeserialize::deserialize(&*public_input).map_err(|e| {
                    log::error!("Deserializing public input failed: {:?}", e);
                    (
                        Error::<T>::DeserializingPublicInputFailed,
                        Some(T::WeightInfo::verify_data_deserializing_fails(
                            proof_len + public_input.len() as u32,
                        )),
                    )
                })?;

            let verification_key =
                VerificationKeys::<T>::get(verification_key_identifier).ok_or((
                    Error::<T>::UnknownKeyPairIdentifier,
                    Some(T::WeightInfo::verify_key_deserializing_fails(0)),
                ))?;
            let verification_key: S::VerifyingKey =
                CanonicalDeserialize::deserialize(&**verification_key).map_err(|e| {
                    log::error!("Deserializing verification key failed: {:?}", e);
                    (
                        Error::<T>::DeserializingVerificationKeyFailed,
                        Some(T::WeightInfo::verify_key_deserializing_fails(
                            verification_key.len() as u32,
                        )),
                    )
                })?;

            let valid_proof = S::verify(&verification_key, &public_input, &proof)
                .map_err(|err| (Error::<T>::VerificationFailed(err), None))?;

            ensure!(valid_proof, (Error::<T>::IncorrectProof, None));

            Self::deposit_event(Event::VerificationSucceeded);
            Ok(())
        }
    }
}
