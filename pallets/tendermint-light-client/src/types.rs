use crate::utils::sha256_from_bytes;
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_std::{time::Duration, vec::Vec};
use tendermint::{
    account,
    block::{self, header::Version, parts::Header as PartSetHeader, Commit, CommitSig, Header},
    chain::{self},
    hash::{self, Hash},
    signature, time,
    validator::Info,
};
use tendermint_light_client_verifier::{
    options::Options,
    types::{LightBlock, PeerId, SignedHeader, TrustThreshold, ValidatorSet},
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

// TODOs:
// * From -> TryFrom everywhere (as the conversion can fail in some instances)

impl TryFrom<LightClientOptionsStorage> for Options {
    type Error = &'static str;

    fn try_from(val: LightClientOptionsStorage) -> Result<Self, Self::Error> {
        Ok(Options {
            trust_threshold: TrustThreshold::new(
                val.trust_threshold.numerator,
                val.trust_threshold.denominator,
            )
            .expect("Can't create TrustThreshold"),
            trusting_period: Duration::from_secs(val.trusting_period),
            clock_drift: Duration::from_secs(val.clock_drift),
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct VersionStorage {
    /// Block version
    pub block: u64,
    /// App version
    pub app: u64,
}

impl From<VersionStorage> for Version {
    fn from(val: VersionStorage) -> Self {
        Version {
            block: val.block,
            app: val.app,
        }
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct PartSetHeaderStorage {
    /// Number of parts in this block
    pub total: u32,
    /// SHA256 Hash of the parts set header,
    pub hash: Vec<u8>,
}

impl TryFrom<PartSetHeaderStorage> for PartSetHeader {
    type Error = &'static str;
    fn try_from(value: PartSetHeaderStorage) -> Result<Self, Self::Error> {
        Ok(
            PartSetHeader::new(value.total, sha256_from_bytes(&value.hash))
                .expect("Can't create PartSetHeader"),
        )
    }
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

impl TryFrom<BlockIdStorage> for block::Id {
    type Error = &'static str;
    fn try_from(value: BlockIdStorage) -> Result<Self, Self::Error> {
        Ok(block::Id {
            hash: sha256_from_bytes(&value.hash),
            part_set_header: value
                .part_set_header
                .try_into()
                .expect("Cannot create block Id"),
        })
    }
}

pub type TendermintAccountId = Vec<u8>; // TODO type enforce length 20?

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
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: u32,
        /// Signature of vote
        signature: Option<SignatureStorage>,
    },
    /// voted for nil.
    BlockIdFlagNil {
        /// Validator address
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: u32,
        /// Signature of vote
        signature: Option<SignatureStorage>,
    },
}

impl CommitSignatureStorage {
    fn validator_address(validator_address: Vec<u8>) -> account::Id {
        account::Id::try_from(validator_address).expect("Cannot create account Id")
    }

    fn timestamp(timestamp: u32) -> time::Time {
        time::Time::from_unix_timestamp(0, timestamp).expect("Cannot parse timestamp")
    }

    fn signature(signature: Option<SignatureStorage>) -> Option<signature::Signature> {
        match signature {
            None => None,
            Some(sig) => signature::Signature::try_from(sig.as_slice()).ok(),
        }
    }
}

impl From<CommitSignatureStorage> for CommitSig {
    fn from(val: CommitSignatureStorage) -> Self {
        match val {
            CommitSignatureStorage::BlockIdFlagAbsent => CommitSig::BlockIdFlagAbsent,
            CommitSignatureStorage::BlockIdFlagCommit {
                validator_address,
                timestamp,
                signature,
            } => {
                let validator_address =
                    CommitSignatureStorage::validator_address(validator_address);
                let timestamp = CommitSignatureStorage::timestamp(timestamp);
                let signature = CommitSignatureStorage::signature(signature);

                CommitSig::BlockIdFlagCommit {
                    validator_address,
                    timestamp,
                    signature,
                }
            }
            CommitSignatureStorage::BlockIdFlagNil {
                validator_address,
                timestamp,
                signature,
            } => {
                let validator_address =
                    CommitSignatureStorage::validator_address(validator_address);
                let timestamp = CommitSignatureStorage::timestamp(timestamp);
                let signature = CommitSignatureStorage::signature(signature);

                CommitSig::BlockIdFlagNil {
                    validator_address,
                    timestamp,
                    signature,
                }
            }
        }
    }
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

#[allow(clippy::from_over_into)]
impl Into<Commit> for CommitStorage {
    fn into(self) -> Commit {
        unimplemented!()
    }
}

// #[allow(clippy::from_over_into)]
impl From<HeaderStorage> for Header {
    fn from(val: HeaderStorage) -> Self {
        unimplemented!()
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct SignedHeaderStorage {
    pub header: HeaderStorage,
    pub commit: CommitStorage,
}

// #[allow(clippy::from_over_into)]
impl From<SignedHeaderStorage> for SignedHeader {
    fn from(val: SignedHeaderStorage) -> Self {
        unimplemented!()
    }
}

pub type TndermintPublicKey = Vec<u8>;

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct ValidatorInfoStorage {
    /// Validator account address
    pub address: TendermintAccountId,
    /// Validator public key
    pub pub_key: TndermintPublicKey,
    /// Validator voting power
    // Compatibility with genesis.json https://github.com/tendermint/tendermint/issues/5549
    #[serde(alias = "voting_power", alias = "total_voting_power")]
    pub power: u64,
    /// Validator name
    pub name: Option<Vec<u8>>,
    /// Validator proposer priority
    #[serde(skip)]
    pub proposer_priority: i64,
}

// #[allow(clippy::from_over_into)]
impl From<ValidatorInfoStorage> for Info {
    fn from(val: ValidatorInfoStorage) -> Self {
        unimplemented!()
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct ValidatorSetStorage {
    pub validators: Vec<ValidatorInfoStorage>,
    pub proposer: Option<ValidatorInfoStorage>,
    pub total_voting_power: u64,
}

// #[allow(clippy::from_over_into)]
impl From<ValidatorSetStorage> for ValidatorSet {
    fn from(val: ValidatorSetStorage) -> Self {
        unimplemented!()
    }
}

pub type TendermintNodeId = Vec<u8>; // TODO type enforce length 20?

#[derive(Encode, Decode, Clone, RuntimeDebug, Serialize, Deserialize, TypeInfo)]
pub struct LightBlockStorage {
    pub signed_header: SignedHeaderStorage,
    pub validators: ValidatorSetStorage,
    pub next_validators: ValidatorSetStorage,
    pub provider: TendermintNodeId,
}

// #[allow(clippy::from_over_into)]
impl From<LightBlockStorage> for LightBlock {
    fn from(val: LightBlockStorage) -> Self {
        unimplemented!()
    }
}
