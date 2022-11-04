use pallet_snarcos::{Error, Pallet as Snarcos, ProvingSystem, VerificationKeyIdentifier};
use sp_std::vec::Vec;

use crate::Runtime;

pub(super) trait Executor: Sized {
    type ErrorGenericType;

    fn store_key(
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
    ) -> Result<(), Error<Self::ErrorGenericType>>;

    fn verify(
        verification_key_identifier: VerificationKeyIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
        system: ProvingSystem,
    ) -> Result<(), Error<Self::ErrorGenericType>>;
}

impl Executor for Runtime {
    type ErrorGenericType = Runtime;

    fn store_key(
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
    ) -> Result<(), Error<Runtime>> {
        Snarcos::<Runtime>::bare_store_key(identifier, key)
    }

    fn verify(
        verification_key_identifier: VerificationKeyIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
        system: ProvingSystem,
    ) -> Result<(), Error<Runtime>> {
        Snarcos::<Runtime>::bare_verify(verification_key_identifier, proof, public_input, system)
    }
}
