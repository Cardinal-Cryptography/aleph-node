use pallet_snarcos::{Error as SnarcosError, ProvingSystem, VerificationKeyIdentifier};

use crate::chain_extension::executor::Executor;

type Error = SnarcosError<()>;
type Result = core::result::Result<(), Error>;

pub(super) struct MockedExecutor<const STORE_KEY_RESPONDER: Result, const VERIFY_RESPONDER: Result>;

impl<const STORE_KEY_RESPONDER: Result, const VERIFY_RESPONDER: Result> Executor
    for MockedExecutor<STORE_KEY_RESPONDER, VERIFY_RESPONDER>
{
    type ErrorGenericType = ();

    fn store_key(_identifier: VerificationKeyIdentifier, _key: Vec<u8>) -> Result {
        STORE_KEY_RESPONDER
    }

    fn verify(
        _verification_key_identifier: VerificationKeyIdentifier,
        _proof: Vec<u8>,
        _public_input: Vec<u8>,
        _system: ProvingSystem,
    ) -> Result {
        VERIFY_RESPONDER
    }
}
