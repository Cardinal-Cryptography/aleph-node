use pallet_snarcos::{Error, ProvingSystem, VerificationKeyIdentifier};

use crate::chain_extension::executor::Executor;

pub(super) struct MockedExecutor;

impl Executor for MockedExecutor {
    fn store_key(_identifier: VerificationKeyIdentifier, _key: Vec<u8>) -> Result<(), Error<Self>> {
        Err(Error::IdentifierAlreadyInUse)
    }

    fn verify(
        _verification_key_identifier: VerificationKeyIdentifier,
        _proof: Vec<u8>,
        _public_input: Vec<u8>,
        _system: ProvingSystem,
    ) -> Result<(), Error<Self>> {
        Err(Error::IncorrectProof)
    }
}
