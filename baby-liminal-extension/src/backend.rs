use frame_support::pallet_prelude::DispatchError;
use log::error;
use pallet_contracts::{
    chain_extension::{
        BufInBufOutState, ChainExtension, Environment, Ext, InitState,
        Result as ChainExtensionResult, RetVal,
    },
    Config as ContractsConfig,
};

use crate::{
    backend_executor::BackendExecutor as BackendExecutorT,
    extension_ids::{STORE_KEY_EXT_ID, VERIFY_EXT_ID},
    status_codes::{STORE_KEY_SUCCESS, VERIFY_SUCCESS},
};

/// The actual implementation of the chain extension. This is the code on the runtime side that will
/// be executed when the chain extension is called.
pub struct BabyLiminalChainExtension<Runtime, BackendExecutor> {
    _config: std::marker::PhantomData<(Runtime, BackendExecutor)>,
}

impl<Runtime: ContractsConfig, BackendExecutor: BackendExecutorT> ChainExtension<Runtime>
    for BabyLiminalChainExtension<Runtime, BackendExecutor>
{
    fn call<E: Ext<T = Runtime>>(
        &mut self,
        env: Environment<E, InitState>,
    ) -> ChainExtensionResult<RetVal> {
        let func_id = env.func_id() as u32;

        match func_id {
            STORE_KEY_EXT_ID => Self::store_key(env.buf_in_buf_out()),
            VERIFY_EXT_ID => Self::verify(env.buf_in_buf_out()),
            _ => {
                error!("Called an unregistered `func_id`: {func_id}");
                Err(DispatchError::Other("Called an unregistered `func_id`"))
            }
        }
    }
}

impl<Runtime: ContractsConfig, BackendExecutor: BackendExecutorT>
    BabyLiminalChainExtension<Runtime, BackendExecutor>
{
    /// Handle `store_key` chain extension call.
    pub fn store_key(
        mut env: Environment<impl Ext<T = Runtime>, BufInBufOutState>,
    ) -> ChainExtensionResult<RetVal> {
        // todo: charge weight, validate args, handle errors
        let args = env.read_as_unbounded(env.in_len())?;
        BackendExecutor::store_key(args)
            .map_err(|_| ())
            .expect("`store_key` failed; this should be handled more gently");
        Ok(RetVal::Converging(STORE_KEY_SUCCESS))
    }

    /// Handle `verify` chain extension call.
    pub fn verify(
        mut env: Environment<impl Ext<T = Runtime>, BufInBufOutState>,
    ) -> ChainExtensionResult<RetVal> {
        // todo: charge weight, validate args, handle errors
        let args = env.read_as_unbounded(env.in_len())?;
        BackendExecutor::verify(args)
            .map_err(|_| ())
            .expect("`verify` failed; this should be handled more gently");
        Ok(RetVal::Converging(VERIFY_SUCCESS))
    }
}
