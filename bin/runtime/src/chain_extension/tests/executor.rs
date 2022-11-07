use pallet_snarcos::{Error as SnarcosError, ProvingSystem, VerificationKeyIdentifier};

use crate::chain_extension::executor::Executor;

type Error = SnarcosError<()>;
type Result = core::result::Result<(), Error>;

#[derive(Clone, Eq, PartialEq)]
pub(super) enum Responder {
    Panicker,
    Okayer,
    Errorer(Error),
}

pub(super) struct MockedExecutor<
    const STORE_KEY_RESPONDER: Responder,
    const VERIFY_RESPONDER: Responder,
>;

/// `struct/enum construction is not supported in generic constants`
pub(super) const fn make_errorer<const ERROR: Error>() -> Responder {
    Responder::Errorer(ERROR)
}

pub(super) type Panicker = MockedExecutor<{ Responder::Panicker }, { Responder::Panicker }>;

pub(super) type StoreKeyOkayer = MockedExecutor<{ Responder::Okayer }, { Responder::Panicker }>;
pub(super) type VerifyOkayer = MockedExecutor<{ Responder::Panicker }, { Responder::Okayer }>;

pub(super) type StoreKeyErrorer<const ERROR: Error> =
    MockedExecutor<{ make_errorer::<ERROR>() }, { Responder::Panicker }>;
pub(super) type VerifyErrorer<const ERROR: Error> =
    MockedExecutor<{ Responder::Panicker }, { make_errorer::<ERROR>() }>;

impl<const STORE_KEY_RESPONDER: Responder, const VERIFY_RESPONDER: Responder> Executor
    for MockedExecutor<STORE_KEY_RESPONDER, VERIFY_RESPONDER>
{
    type ErrorGenericType = ();

    fn store_key(_identifier: VerificationKeyIdentifier, _key: Vec<u8>) -> Result {
        match STORE_KEY_RESPONDER {
            Responder::Panicker => panic!("Function `store_key` shouldn't have been executed"),
            Responder::Okayer => Ok(()),
            Responder::Errorer(e) => Err(e),
        }
    }

    fn verify(
        _verification_key_identifier: VerificationKeyIdentifier,
        _proof: Vec<u8>,
        _public_input: Vec<u8>,
        _system: ProvingSystem,
    ) -> Result {
        match VERIFY_RESPONDER {
            Responder::Panicker => panic!("Function `verify` shouldn't have been executed"),
            Responder::Okayer => Ok(()),
            Responder::Errorer(e) => Err(e),
        }
    }
}
