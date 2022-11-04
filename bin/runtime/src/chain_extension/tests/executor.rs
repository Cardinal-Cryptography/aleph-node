use std::marker::PhantomData;

use pallet_snarcos::{Error as SnarcosError, ProvingSystem, VerificationKeyIdentifier};

use crate::chain_extension::executor::Executor;

type Error = SnarcosError<()>;
type Result = core::result::Result<(), Error>;

pub(super) struct MockedExecutor<const StoreKeyResponder: Result, const VerifyResponder: Result>;

impl<const StoreKeyResponder: Result, const VerifyResponder: Result> Executor
    for MockedExecutor<StoreKeyResponder, VerifyResponder>
{
    type ErrorGenericType = ();

    fn store_key(_identifier: VerificationKeyIdentifier, _key: Vec<u8>) -> Result {
        StoreKeyResponder
    }

    fn verify(
        _verification_key_identifier: VerificationKeyIdentifier,
        _proof: Vec<u8>,
        _public_input: Vec<u8>,
        _system: ProvingSystem,
    ) -> Result {
        VerifyResponder
    }
}
