use pallet_snarcos::{Error, Pallet as Snarcos, ProvingSystem, VerificationKeyIdentifier};
use sp_std::vec::Vec;

use crate::Runtime;

pub(super) trait Executor: Sized {
    fn store_key(identifier: VerificationKeyIdentifier, key: Vec<u8>) -> Result<(), Error<Self>>;

    fn verify(
        verification_key_identifier: VerificationKeyIdentifier,
        proof: Vec<u8>,
        public_input: Vec<u8>,
        system: ProvingSystem,
    ) -> Result<(), Error<Self>>;
}

impl Executor for Runtime {
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
