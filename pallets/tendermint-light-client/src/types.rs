use crate::{
    utils::{account_id_from_bytes, from_unix_timestamp, sha256_from_bytes},
    Error,
};
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use scale_info::{prelude::string::String, TypeInfo};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{H160, H256, H512};
use sp_std::{borrow::ToOwned, time::Duration, vec::Vec};
use tendermint::{
    block::{self, header::Version, parts::Header as PartSetHeader, Commit, CommitSig, Header},
    chain, hash, node, public_key,
    serializers::bytes,
    signature,
    validator::{self, ProposerPriority},
    vote,
};
use tendermint_light_client_verifier::{
    options::Options,
    types::{LightBlock, SignedHeader, TrustThreshold, ValidatorSet},
};

pub type TendermintVoteSignature = String; // TODO base64 encoded
pub type AppHashStorage = String; // TODO this is fuzzy
pub type TendermintPeerId = H160;
pub type TendermintAccountId = H160;
pub type Hash = H256;
pub type BridgedBlockHash = Hash;

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TrustThresholdStorage {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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
        Self {
            trust_threshold: TrustThresholdStorage {
                numerator: 1,
                denominator: 3,
            },
            trusting_period: 1210000, // 2 weeks
            clock_drift: 5,
        }
    }
}

impl TryFrom<LightClientOptionsStorage> for Options {
    type Error = &'static str;

    fn try_from(val: LightClientOptionsStorage) -> Result<Self, Self::Error> {
        Ok(Self {
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

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PartSetHeaderStorage {
    /// Number of parts in this block
    pub total: u32,
    /// SHA256 Hash of the parts set header,
    pub hash: Hash,
}

impl TryFrom<PartSetHeaderStorage> for PartSetHeader {
    type Error = &'static str;
    fn try_from(value: PartSetHeaderStorage) -> Result<Self, Self::Error> {
        Ok(
            PartSetHeader::new(value.total, sha256_from_bytes(value.hash.as_bytes()))
                .expect("Can't create PartSetHeader"),
        )
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BlockIdStorage {
    /// The block's main hash is the Merkle root of all the fields in the
    /// block header.
    pub hash: BridgedBlockHash,
    /// Parts header (if available) is used for secure gossipping of the block
    /// during consensus. It is the Merkle root of the complete serialized block
    /// cut into parts.
    pub part_set_header: PartSetHeaderStorage,
}

impl TryFrom<BlockIdStorage> for block::Id {
    type Error = &'static str;
    fn try_from(value: BlockIdStorage) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: sha256_from_bytes(value.hash.as_bytes()),
            part_set_header: value
                .part_set_header
                .try_into()
                .expect("Cannot create block Id"),
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct HeaderStorage {
    /// Header version
    pub version: VersionStorage,
    /// Chain identifier (e.g. 'gaia-9000')    
    pub chain_id: String, // Vec<u8>, // String::from_utf8,
    /// Current block height
    pub height: u64,
    /// Epoch Unix timestamp in seconds
    #[cfg_attr(feature = "std", serde(alias = "time", alias = "timestamp"))]
    pub timestamp: i64,
    /// Previous block info
    pub last_block_id: Option<BlockIdStorage>,
    /// Commit from validators from the last block
    pub last_commit_hash: Option<Hash>,
    /// Merkle root of transaction hashes
    pub data_hash: Option<Hash>,
    /// Validators for the current block
    pub validators_hash: Hash, //Vec<u8>,
    /// Validators for the next block
    pub next_validators_hash: Hash,
    /// Consensus params for the current block
    pub consensus_hash: Hash,
    /// State after txs from the previous block
    /// AppHash is usually a SHA256 hash, but in reality it can be any kind of data    
    pub app_hash: AppHashStorage,
    /// Root hash of all results from the txs from the previous block
    pub last_results_hash: Option<Hash>,
    /// Hash of evidence included in the block
    pub evidence_hash: Option<Hash>,
    /// Original proposer of the block
    pub proposer_address: TendermintAccountId,
}

/// CommitSig represents a signature of a validator.
/// It's a part of the Commit and can be used to reconstruct the vote set given the validator set.
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(tag = "block_id_flag"))]
pub enum CommitSignatureStorage {
    /// no vote was received from a validator.
    BlockIdFlagAbsent,
    /// voted for the Commit.BlockID.
    BlockIdFlagCommit {
        /// Validator address
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: i64,
        /// Signature of vote
        signature: Option<TendermintVoteSignature>,
    },
    /// voted for nil.
    BlockIdFlagNil {
        /// Validator address
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: i64,
        /// Signature of vote
        signature: Option<TendermintVoteSignature>,
    },
}

impl TryFrom<CommitSignatureStorage> for CommitSig {
    type Error = &'static str;

    fn try_from(value: CommitSignatureStorage) -> Result<Self, Self::Error> {
        Ok(match value {
            CommitSignatureStorage::BlockIdFlagAbsent => Self::BlockIdFlagAbsent,
            CommitSignatureStorage::BlockIdFlagCommit {
                validator_address,
                timestamp,
                signature,
            } => {
                let validator_address =
                    account_id_from_bytes(validator_address.as_fixed_bytes().to_owned());
                let timestamp = from_unix_timestamp(timestamp);

                let signature = CommitSignatureStorage::signature(signature);

                Self::BlockIdFlagCommit {
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
                    account_id_from_bytes(validator_address.as_fixed_bytes().to_owned());
                let timestamp = from_unix_timestamp(timestamp);

                let signature = CommitSignatureStorage::signature(signature);

                Self::BlockIdFlagNil {
                    validator_address,
                    timestamp,
                    signature,
                }
            }
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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

impl CommitSignatureStorage {
    fn signature(signature: Option<TendermintVoteSignature>) -> Option<tendermint::Signature> {
        match signature {
            None => None,
            Some(base64) => {
                let bytes = base64::decode(base64).expect("Not base64 encoded");
                tendermint::Signature::try_from(bytes).ok()
            }
        }
    }
}

impl TryFrom<CommitStorage> for Commit {
    type Error = &'static str;

    fn try_from(value: CommitStorage) -> Result<Self, Self::Error> {
        let CommitStorage {
            height,
            round,
            block_id,
            signatures,
        } = value;

        Ok(Self {
            height: block::Height::try_from(height).expect("Cannot create Height"),
            round: block::Round::try_from(round).expect("Cannot create Round"),
            block_id: block_id.try_into().expect("Cannot create block Id"),
            signatures: signatures
                .into_iter()
                .map(|elem| elem.try_into().expect("Cannot create Commit"))
                .collect(),
        })
    }
}

impl TryFrom<HeaderStorage> for Header {
    type Error = &'static str;

    fn try_from(value: HeaderStorage) -> Result<Self, Self::Error> {
        let HeaderStorage {
            version,
            chain_id,
            height,
            timestamp,
            last_block_id,
            last_commit_hash,
            data_hash,
            validators_hash,
            next_validators_hash,
            consensus_hash,
            app_hash,
            last_results_hash,
            evidence_hash,
            proposer_address,
        } = value;

        Ok(Self {
            version: version.try_into().expect("Cannot create Version"),
            chain_id: chain_id
                .parse::<chain::Id>()
                .expect("Cannot parse as Chain Id"),
            // String::from_utf8(chain_id)
            //     .expect("Not a UTF8 string encoding")
            //     .parse::<chain::Id>()
            //     .expect("Cannot parse as Chain Id"),
            height: block::Height::try_from(height).expect("Cannot create Height"),
            time: from_unix_timestamp(timestamp),
            last_block_id: match last_block_id {
                Some(id) => id.try_into().ok(),
                None => None,
            },
            last_commit_hash: match last_commit_hash {
                Some(hash) => Some(sha256_from_bytes(hash.as_bytes())),
                None => None,
            },
            data_hash: match data_hash {
                Some(hash) => Some(sha256_from_bytes(hash.as_bytes())),
                None => None,
            },
            validators_hash: sha256_from_bytes(validators_hash.as_bytes()),
            next_validators_hash: sha256_from_bytes(next_validators_hash.as_bytes()),
            consensus_hash: sha256_from_bytes(consensus_hash.as_bytes()),
            app_hash: hash::AppHash::from_hex_upper(&app_hash).expect("Cannot create AppHash"),
            //hash::AppHash::try_from(app_hash).expect("Cannot create AppHash"),
            last_results_hash: match last_results_hash {
                Some(hash) => Some(sha256_from_bytes(&hash.as_bytes())),
                None => None,
            },
            evidence_hash: match evidence_hash {
                Some(hash) => Some(sha256_from_bytes(hash.as_bytes())),
                None => None,
            },
            proposer_address: account_id_from_bytes(proposer_address.as_fixed_bytes().to_owned()),
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct SignedHeaderStorage {
    pub header: HeaderStorage,
    pub commit: CommitStorage,
}

impl TryFrom<SignedHeaderStorage> for SignedHeader {
    type Error = &'static str;

    fn try_from(value: SignedHeaderStorage) -> Result<Self, Self::Error> {
        let SignedHeaderStorage { header, commit } = value;

        Ok(Self::new(
            header.try_into().expect("Cannot create Header"),
            commit.try_into().expect("Cannot create Commit"),
        )
        .expect("Cannot create SignedHeader"))
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(tag = "type", content = "value"))]
pub enum TendermintPublicKey {
    #[cfg_attr(feature = "std", serde(rename = "tendermint/PubKeyEd25519"))]
    Ed25519(String),
    #[cfg_attr(feature = "std", serde(rename = "tendermint/PubKeySecp256k1"))]
    Secp256k1(String),
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ValidatorInfoStorage {
    /// Validator account address
    pub address: TendermintAccountId,
    /// Validator public key
    pub pub_key: TendermintPublicKey,
    /// Validator voting power
    // Compatibility with genesis.json https://github.com/tendermint/tendermint/issues/5549
    #[cfg_attr(
        feature = "std",
        serde(alias = "voting_power", alias = "total_voting_power")
    )]
    pub power: u64,
    /// Validator name
    pub name: Option<Vec<u8>>,
    /// Validator proposer priority
    #[cfg_attr(feature = "std", serde(skip))]
    pub proposer_priority: i64,
}

impl TryFrom<ValidatorInfoStorage> for validator::Info {
    type Error = &'static str;

    fn try_from(value: ValidatorInfoStorage) -> Result<Self, Self::Error> {
        let ValidatorInfoStorage {
            address,
            pub_key,
            power,
            name,
            proposer_priority,
        } = value;

        Ok(Self {
            address: account_id_from_bytes(address.as_fixed_bytes().to_owned()),
            pub_key: match pub_key {
                TendermintPublicKey::Ed25519(base64) => {
                    // let bytes = base64::decode(base64).expect("Not base64 encoded");
                    // tendermint::PublicKey::from_raw_ed25519(&bytes).expect("Not ed25519 public key")

                    unimplemented!()
                }
                TendermintPublicKey::Secp256k1(base64) => {
                    // let bytes = base64::decode(base64).expect("Not base64 encoded");
                    // tendermint::PublicKey::from_raw_secp256k1(&bytes)
                    //     .expect("Not secp256k1 public key")

                    unimplemented!()
                }
            },
            power: vote::Power::try_from(power).expect("Cannot create Power"),
            name: match name {
                Some(name) => String::from_utf8(name).ok(),
                None => None,
            },
            proposer_priority: ProposerPriority::from(proposer_priority),
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ValidatorSetStorage {
    pub validators: Vec<ValidatorInfoStorage>,
    pub proposer: Option<ValidatorInfoStorage>,
    pub total_voting_power: u64,
}

impl TryFrom<ValidatorSetStorage> for ValidatorSet {
    type Error = &'static str;

    fn try_from(value: ValidatorSetStorage) -> Result<Self, Self::Error> {
        let ValidatorSetStorage {
            validators,
            proposer,
            ..
        } = value;

        Ok(Self::new(
            validators
                .into_iter()
                .map(|elem| elem.try_into().expect("Cannot create ValidatorInfo"))
                .collect(),
            match proposer {
                Some(proposer) => proposer.try_into().ok(),
                None => None,
            },
        ))
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LightBlockStorage {
    /// Header and commit of this block    
    pub signed_header: SignedHeaderStorage,
    /// Validator set at the block height
    #[cfg_attr(feature = "std", serde(rename = "validator_set"))]
    pub validators: ValidatorSetStorage,
    /// Validator set at the next block height
    #[cfg_attr(feature = "std", serde(rename = "next_validator_set"))]
    pub next_validators: ValidatorSetStorage,
    /// The peer (noide) ID of the node that provided this block    
    pub provider: TendermintPeerId,
}

impl TryFrom<LightBlockStorage> for LightBlock {
    type Error = &'static str;

    fn try_from(value: LightBlockStorage) -> Result<Self, Self::Error> {
        let LightBlockStorage {
            signed_header,
            validators,
            next_validators,
            provider,
        } = value;

        let bytes: [u8; 20] = provider.try_into().expect("Not a 20 byte array");

        Ok(Self {
            signed_header: signed_header
                .try_into()
                .expect("Cannot create SignedHeader"),
            validators: validators.try_into().expect("Cannot create ValidatorSet"),
            next_validators: next_validators
                .try_into()
                .expect("Cannot create next ValidatorSet"),
            provider: node::Id::new(bytes),
        })
    }
}
