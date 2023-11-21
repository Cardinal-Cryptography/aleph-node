use frame_support::{__private::log, pallet_prelude::DispatchError};
use log::error;
use pallet_contracts::chain_extension::{ChainExtension, Environment, Ext, InitState, RetVal};

use crate::extension_ids::{STORE_KEY_EXT_ID, VERIFY_EXT_ID};

/// The actual implementation of the chain extension. This is the code on the runtime side that will
/// be executed when the chain extension is called.
pub struct BabyLiminalChainExtension<PalletConfig> {
    _config: std::marker::PhantomData<PalletConfig>,
}

impl<PalletConfig: pallet_contracts::Config> ChainExtension<PalletConfig>
    for BabyLiminalChainExtension<PalletConfig>
{
    fn call<E: Ext<T = PalletConfig>>(
        &mut self,
        env: Environment<E, InitState>,
    ) -> pallet_contracts::chain_extension::Result<RetVal> {
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
