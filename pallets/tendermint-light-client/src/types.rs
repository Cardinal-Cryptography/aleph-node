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

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct PartSetHeaderStorage {
    /// Number of parts in this block
    pub total: u32,
    /// SHA256 Hash of the parts set header,
    pub hash: Vec<u8>,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct BlockIdStorage {
    /// The block's main hash is the Merkle root of all the fields in the
    /// block header.
    pub hash: Vec<u8>,
    /// Parts header (if available) is used for secure gossipping of the block
    /// during consensus. It is the Merkle root of the complete serialized block
    /// cut into parts.
    pub part_set_header: PartSetHeaderStorage,
}

pub type TendermintAccountId = Vec<u8>; // TODO type enforce length 620?

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct HeaderStorage {
    /// Header version
    pub version: VersionStorage,
    /// Chain identifier (e.g. 'gaia-9000')    
    pub chain_id: Vec<u8>, // String::from_utf8,
    /// Current block height
    pub height: u64,
    /// Current timestamp in nanoseconds
    pub time: u32,
    /// Previous block info
    pub last_block_id: Option<BlockIdStorage>,
    /// Commit from validators from the last block
    pub last_commit_hash: Option<Vec<u8>>,
    /// Merkle root of transaction hashes
    pub data_hash: Option<Vec<u8>>,
    /// Validators for the current block
    pub validators_hash: Vec<u8>,
    /// Validators for the next block
    pub next_validators_hash: Vec<u8>,
    /// Consensus params for the current block
    pub consensus_hash: Vec<u8>,
    /// State after txs from the previous block
    /// AppHash is usually a SHA256 hash, but in reality it can be any kind of data    
    pub app_hash: Vec<u8>,
    /// Root hash of all results from the txs from the previous block
    pub last_results_hash: Option<Vec<u8>>,
    /// Hash of evidence included in the block
    pub evidence_hash: Option<Vec<u8>>,
    /// Original proposer of the block
    pub proposer_address: TendermintAccountId,
}

pub type SignatureStorage = Vec<u8>; // TODO type enforce length 64?

/// CommitSig represents a signature of a validator.
/// It's a part of the Commit and can be used to reconstruct the vote set given the validator set.
#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub enum CommitSignatureStorage {
    /// no vote was received from a validator.
    BlockIdFlagAbsent,
    /// voted for the Commit.BlockID.
    BlockIdFlagCommit {
        /// Validator address
        validator_address: [u8; 20],
        /// Timestamp of vote
        timestamp: u32,
        /// Signature of vote
        signature: Option<SignatureStorage>,
    },
    /// voted for nil.
    BlockIdFlagNil {
        /// Validator address
        validator_address: [u8; 20],
        /// Timestamp of vote
        timestamp: u32,
        /// Signature of vote
        signature: Option<SignatureStorage>,
    },
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct CommitStorage {
    /// Block height
    pub height: u64,
    /// Round
    pub round: u32,
    /// Block ID
    pub block_id: BlockIdStorage,
    /// Signatures
    pub signatures: Vec<CommitSignatureStorage>,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct SignedHeaderStorage {
    pub header: HeaderStorage,
    pub commit: CommitStorage,
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
