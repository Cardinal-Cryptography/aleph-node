use obce::substrate::{
    frame_support::weights::Weight,
    frame_system::Config as SysConfig,
    pallet_contracts::chain_extension::{BufInBufOutState, Environment, Ext},
};
use pallet_baby_liminal::{Config as BabyLiminalConfig, Error, Pallet as BabyLiminal};

use crate::{AccountId32, KeyPairIdentifier, Vec};

/// Generalized pallet executor, that can be mocked for testing purposes.
pub trait Executor<T>: Sized {
    /// The error returned from dispatchables is generic. For most purposes however, it doesn't
    /// matter what type will be passed there. Normally, `Runtime` will be the generic argument,
    /// but in testing it will be sufficient to instantiate it with `()`.
    type ErrorGenericType;

    fn store_key_pair(
        depositor: AccountId32,
        identifier: KeyPairIdentifier,
        proving_key: Vec<u8>,
        verification_key: Vec<u8>,
    ) -> Result<(), Error<Self::ErrorGenericType>>;

    fn verify(
        identifier: KeyPairIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
    ) -> Result<(), (Error<Self::ErrorGenericType>, Option<Weight>)>;
}

impl<'a, 'b, E, T> Executor<T> for Environment<'a, 'b, E, BufInBufOutState>
where
    T: SysConfig + BabyLiminalConfig,
    E: Ext<T = T>,
    <T as SysConfig>::RuntimeOrigin: From<Option<AccountId32>>,
{
    type ErrorGenericType = T;

    fn store_key_pair(
        depositor: AccountId32,
        identifier: KeyPairIdentifier,
        proving_key: Vec<u8>,
        verification_key: Vec<u8>,
    ) -> Result<(), Error<Self::ErrorGenericType>> {
        BabyLiminal::<T>::bare_store_key_pair(
            Some(depositor).into(),
            identifier,
            proving_key,
            verification_key,
        )
    }

    fn verify(
        identifier: KeyPairIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
    ) -> Result<(), (Error<Self::ErrorGenericType>, Option<Weight>)> {
        BabyLiminal::<T>::bare_verify(identifier, proof, public_input)
    }
}
