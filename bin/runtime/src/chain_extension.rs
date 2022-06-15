use crate::{AccountId, Balance, Runtime};
use codec::{Decode, Encode};
use frame_support::{log, storage::migration::get_storage_value, storage_alias, Twox64Concat};
use pallet_contracts::chain_extension::{
    ChainExtension, Environment, Ext, InitState, RetVal, SysConfig, UncheckedFrom,
};
use sp_runtime::DispatchError;

// use pallet_contracts::{wasm::OwnerInfo, OwnerInfoOf};
// use pallet_contracts::pallet::OwnerInfoOf;

#[derive(Clone, Encode, Decode)]
pub struct OwnerInfo {
    owner: AccountId,
    #[codec(compact)]
    deposit: Balance,
    #[codec(compact)]
    nonce: u64,
}

// #[storage_alias]
// type ContractsOwnerInfo = StorageMap<Contracts, AccountId, OwnerInfo, Twox64Concat>;

// #[storage_alias]
// type ContractInfoOf<T: Config> =
//     StorageMap<Pallet<T>, Twox64Concat, <T as frame_system::Config>::AccountId, ContractInfo<T>>;

#[storage_alias]
type OwnerInfoOf = StorageMap<Contracts, Twox64Concat, AccountId, OwnerInfo>;

/// Contract extension for getting owner info of a code hash
pub struct ContractsOwnerInfoOf;

impl ChainExtension<Runtime> for ContractsOwnerInfoOf {
    fn call<E: Ext>(func_id: u32, env: Environment<E, InitState>) -> Result<RetVal, DispatchError>
    where
        <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
    {
        log::debug!(target: "extension", "Chain extension called");

        match func_id {
            1101 => {
                let mut env = env.buf_in_buf_out();
                let arg: AccountId = env.read_as()?;

                log::debug!(target: "extension", "Chain extension called for contract {:?}", &arg);

                let info = OwnerInfoOf::try_get(arg);

                if let Ok(owner_info) = &info {
                    log::debug!(target: "extension", "Chain extension retrieved ownership info {:?}", &owner_info.owner);
                } else {
                    log::debug!(target: "extension", "No ownership info found");
                }

                env.write(&info.encode(), false, None).map_err(|_| {
                    DispatchError::Other("ChainExtension failed to read contract's owner info")
                })?;
            }

            _ => {
                log::error!("Called an unregistered `func_id`: {:}", func_id);
                return Err(DispatchError::Other("Unimplemented func_id"));
            }
        }
        // chain extensions returns the value to its calling contract
        Ok(RetVal::Converging(0))
    }

    fn enabled() -> bool {
        true
    }
}
