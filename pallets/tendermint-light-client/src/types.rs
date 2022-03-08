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
use time::{OffsetDateTime, PrimitiveDateTime};

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct TrustThresholdStorage {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct LightClientOptionsStorage {
    /// Defines what fraction of the total voting power of a known
    /// and trusted validator set is sufficient for a commit to be
    /// accepted going forward.    
    pub trust_threshold: TrustThresholdStorage,
    /// How long a validator set is trusted for (must be shorter than the chain's
    /// unbonding period)    
    pub trusting_period: u64,
    /// Correction parameter dealing with only approximately synchronized clocks.
    /// The local clock should always be ahead of timestamps from the blockchain; this
    /// is the maximum amount that the local clock may drift behind a timestamp from the
    /// blockchain.    
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

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct VersionStorage {
    /// Block version
    pub block: u64,
    /// App version
    pub app: u64,
}

// #[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
// pub struct ChainIdStorage(String);

// #[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
// pub struct HeightStorage(u64);

// #[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
// pub struct Time(PrimitiveDateTime);

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct HeaderStorage {
    /// Header version
    pub version: VersionStorage,
    /// Chain identifier (e.g. 'gaia-9000')    
    pub chain_id: Vec<u8>, // String::from_utf8,
    /// Current block height
    pub height: u64,
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

#[allow(clippy::from_over_into)]
impl Into<LightBlock> for LightBlockStorage {
    fn into(self) -> LightBlock {
        unimplemented!()
    }
}
