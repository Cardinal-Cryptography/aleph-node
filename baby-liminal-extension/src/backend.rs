use frame_support::pallet_prelude::DispatchError;
use log::error;
use pallet_contracts::chain_extension::{
    ChainExtension, Environment, Ext, InitState, Result as ChainExtensionResult, RetVal,
};

use crate::extension_ids::{STORE_KEY_EXT_ID, VERIFY_EXT_ID};

/// The actual implementation of the chain extension. This is the code on the runtime side that will
/// be executed when the chain extension is called.
pub struct BabyLiminalChainExtension<ContractsConfig> {
    _config: std::marker::PhantomData<ContractsConfig>,
}

impl<ContractsConfig: pallet_contracts::Config> ChainExtension<ContractsConfig>
    for BabyLiminalChainExtension<ContractsConfig>
{
    fn call<E: Ext<T = ContractsConfig>>(
        &mut self,
        env: Environment<E, InitState>,
    ) -> ChainExtensionResult<RetVal> {
        let func_id = env.func_id() as u32;

        match func_id {
            STORE_KEY_EXT_ID => todo!(),
            VERIFY_EXT_ID => todo!(),
            _ => {
                error!("Called an unregistered `func_id`: {func_id}");
                Err(DispatchError::Other("Called an unregistered `func_id`"))
            }
        }
    }
}
