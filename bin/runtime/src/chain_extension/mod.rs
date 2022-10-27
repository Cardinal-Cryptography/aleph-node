use frame_support::log::error;
use pallet_contracts::chain_extension::{
    ChainExtension, Environment, Ext, InitState, RetVal, SysConfig,
};
use pallet_snarcos::{Error, Pallet as Snarcos};
use sp_core::crypto::UncheckedFrom;
use sp_runtime::DispatchError;

use crate::Runtime;

pub const SNARCOS_CHAIN_EXT: u32 = 41;

// Return codes.
pub const SNARCOS_STORE_KEY_OK: u32 = 0;
pub const SNARCOS_STORE_KEY_TOO_LONG_KEY: u32 = 1;
pub const SNARCOS_STORE_KEY_IN_USE: u32 = 2;
pub const SNARCOS_STORE_KEY_UNKNOWN: u32 = 3;

pub struct AlephChainExtension;

impl ChainExtension<Runtime> for AlephChainExtension {
    fn call<E: Ext>(func_id: u32, env: Environment<E, InitState>) -> Result<RetVal, DispatchError>
    where
        <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
    {
        match func_id {
            SNARCOS_CHAIN_EXT => Self::snarcos_store_key(env),
            _ => {
                error!("Called an unregistered `func_id`: {}", func_id);
                Err(DispatchError::Other("Unimplemented func_id"))
            }
        }
    }
}

impl AlephChainExtension {
    fn snarcos_store_key<E: Ext>(env: Environment<E, InitState>) -> Result<RetVal, DispatchError>
    where
        <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
    {
        let mut env = env.buf_in_buf_out();
        env.charge_weight(41)?;

        let return_status = match Snarcos::<Runtime>::bare_store_key([0u8; 4], [0u8; 8].to_vec()) {
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
