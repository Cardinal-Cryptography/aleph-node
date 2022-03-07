use codec::{Decode, Encode, WrapperTypeDecode};
use frame_support::{
    log,
    pallet_prelude::{DispatchClass, DispatchResult, IsType, StorageValue, ValueQuery},
    traits::Get,
    RuntimeDebug,
};
use frame_system::{
    ensure_root,
    pallet_prelude::{BlockNumberFor, OriginFor},
};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_std::{time::Duration, vec::Vec};
use tendermint_light_client_verifier::{
    options::Options,
    types::{LightBlock, TrustThreshold},
    ProdVerifier,
};

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct TrustThresholdStorage {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct LightClientOptionsStorage {
    pub trust_threshold: TrustThresholdStorage,
    pub trusting_period: u64,
    pub clock_drift: u64,
}

impl Default for LightClientOptionsStorage {
    fn default() -> Self {
        LightClientOptionsStorage {
            trust_threshold: TrustThresholdStorage {
                numerator: 1,
                denominator: 3,
            },
            trusting_period: 1210000, // 2 weeks
            clock_drift: 5,
        }
    }
}

impl Into<Options> for LightClientOptionsStorage {
    fn into(self) -> Options {
        Options {
            trust_threshold: TrustThreshold::new(
                self.trust_threshold.numerator,
                self.trust_threshold.denominator,
            )
            .expect("Can't create TrustThreshold"),
            trusting_period: Duration::from_secs(self.trusting_period),
            clock_drift: Duration::from_secs(self.clock_drift),
        }
    }
}
