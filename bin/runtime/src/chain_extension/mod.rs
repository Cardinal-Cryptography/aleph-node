use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{dispatch::Weight, log::error};
use pallet_contracts::chain_extension::{
    ChainExtension, Environment, Ext, InitState, RetVal, SysConfig,
};
use pallet_snarcos::{Config, Error, Pallet as Snarcos, VerificationKeyIdentifier, WeightInfo};
use scale_info::TypeInfo;
use sp_core::crypto::UncheckedFrom;
use sp_runtime::{traits::Get, BoundedVec, DispatchError};

use crate::{MaximumVerificationKeyLength, Runtime};

pub const SNARCOS_STORE_KEY_FUNC_ID: u32 = 41;

// Return codes.
pub const SNARCOS_STORE_KEY_OK: u32 = 0;
pub const SNARCOS_STORE_KEY_TOO_LONG_KEY: u32 = 1;
pub const SNARCOS_STORE_KEY_IN_USE: u32 = 2;
pub const SNARCOS_STORE_KEY_UNKNOWN: u32 = 3;

pub struct SnarcosChainExtension;

impl ChainExtension<Runtime> for SnarcosChainExtension {
    fn call<E: Ext>(func_id: u32, env: Environment<E, InitState>) -> Result<RetVal, DispatchError>
    where
        <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
    {
        match func_id {
            SNARCOS_STORE_KEY_FUNC_ID => Self::snarcos_store_key(env),
            _ => {
                error!("Called an unregistered `func_id`: {}", func_id);
                Err(DispatchError::Other("Unimplemented func_id"))
            }
        }
    }
}

pub type ByteCount = u32;

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode, MaxEncodedLen, TypeInfo)]
pub struct StoreKeyArgs<S: Get<ByteCount>> {
    pub identifier: VerificationKeyIdentifier,
    pub key: BoundedVec<u8, S>,
}

impl SnarcosChainExtension {
    fn store_key_weight(key_length: ByteCount) -> Weight {
        <<Runtime as Config>::WeightInfo as WeightInfo>::store_key(key_length)
    }

    fn snarcos_store_key<E: Ext>(env: Environment<E, InitState>) -> Result<RetVal, DispatchError>
    where
        <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
    {
        // We need to read input as plain bytes (encoded args).
        let mut env = env.buf_in_buf_out();

        // Pre-charge caller with the maximum weight, because we have no idea now how much is there
        // to read.
        let pre_charged =
            env.charge_weight(Self::store_key_weight(MaximumVerificationKeyLength::get()))?;
        // Decode arguments.
        let args = env.read_as::<StoreKeyArgs<MaximumVerificationKeyLength>>()?;
        // In case the key was shorter than the limit, we give back paid overhead.
        env.adjust_weight(
            pre_charged,
            Self::store_key_weight(args.key.len() as ByteCount),
        );

        let return_status =
            match Snarcos::<Runtime>::bare_store_key(args.identifier, args.key.into_inner()) {
                Ok(_) => SNARCOS_STORE_KEY_OK,
                // In case `DispatchResultWithPostInfo` was returned (or some simpler equivalent for
                // `bare_store_key`), we could adjust weight. However, for the storing key action it
                // doesn't make sense.
                Err(Error::<Runtime>::VerificationKeyTooLong) => SNARCOS_STORE_KEY_TOO_LONG_KEY,
                Err(Error::<Runtime>::IdentifierAlreadyInUse) => SNARCOS_STORE_KEY_IN_USE,
                _ => SNARCOS_STORE_KEY_UNKNOWN,
            };
        Ok(RetVal::Converging(return_status))
    }
}
