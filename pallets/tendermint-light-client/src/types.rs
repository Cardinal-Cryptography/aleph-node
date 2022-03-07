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
    types::{LightBlock, PeerId, SignedHeader, TrustThreshold, ValidatorSet},
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

#[allow(clippy::from_over_into)]
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

// impl From<Options> for LightClientOptionsStorage {
//     fn from(opts: Options) -> Self {
//         Self {
//             trust_threshold: TrustThresholdStorage {
//                 denominator: opts.trust_threshold.denominator(),
//                 numerator: opts.trust_threshold.numerator(),
//             },
//             trusting_period: opts.trusting_period.as_secs(),
//             clock_drift: opts.clock_drift.as_secs(),
//         }
//     }
// }

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct VersionStorage {
    /// Block version
    pub block: u64,

    /// App version
    pub app: u64,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct HeaderStorage {
    /// Header version
    pub version: VersionStorage,
    // /// Chain ID
    // pub chain_id: chain::Id,

    // /// Current block height
    // pub height: block::Height,

    // /// Current timestamp
    // pub time: Time,

    // /// Previous block info
    // pub last_block_id: Option<block::Id>,

    // /// Commit from validators from the last block
    // pub last_commit_hash: Option<Hash>,

    // /// Merkle root of transaction hashes
    // pub data_hash: Option<Hash>,

    // /// Validators for the current block
    // pub validators_hash: Hash,

    // /// Validators for the next block
    // pub next_validators_hash: Hash,

    // /// Consensus params for the current block
    // pub consensus_hash: Hash,

    // /// State after txs from the previous block
    // pub app_hash: AppHash,

    // /// Root hash of all results from the txs from the previous block
    // pub last_results_hash: Option<Hash>,

    // /// Hash of evidence included in the block
    // pub evidence_hash: Option<Hash>,

    // /// Original proposer of the block
    // pub proposer_address: account::Id,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct SignedHeaderStorage {
    pub header: HeaderStorage,
    // pub commit: Commit,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct LightBlockStorage {
    pub signed_header: SignedHeaderStorage,
    // pub validators: ValidatorSet,
    // pub next_validators: ValidatorSet,
    // pub provider: PeerId,
}
