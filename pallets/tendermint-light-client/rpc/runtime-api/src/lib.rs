//! Runtime API definition for tendermint LC pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;
pub use pallet_tendermint_light_client::types::LightBlockStorage;

sp_api::decl_runtime_apis! {
    pub trait TendermintLightClientApi
    {
        fn get_last_imported_block() -> Option<LightBlockStorage>;
    }
}
