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

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize)]
pub struct TrustThresholdStorage {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize)]
pub struct LightClientOptionsStorage {
    pub trust_threshold: TrustThresholdStorage,
    pub trusting_period: Duration,
    pub clock_drift: Duration,
}

impl Default for LightClientOptionsStorage {
    fn default() -> Self {
        LightClientOptionsStorage {
            trust_threshold: TrustThresholdStorage {
                numerator: 1,
                denominator: 3,
            },
            trusting_period: Duration::new(1210000, 0), // 2 weeks
            clock_drift: Duration::new(5, 0),
        }
    }
}

impl Into<Options> for LightClientOptionsStorage {
    fn into(opts: LightClientOptionsStorage) -> Options {
        Options {
            trust_threshold: TrustThreshold {
                numerator : opts.trust_threshold.numerator,
                denominator: opts.trust_threshold.denominator
            },
            trusting_period: opts.trusting_period,
            clock_drift: opts.clock_drift
        }                
    }
}
