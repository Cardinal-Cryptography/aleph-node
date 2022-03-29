use crate::utils::{account_id_from_bytes, sha256_from_bytes};
#[cfg(feature = "std")]
use crate::utils::{
    base64string_as_h512, deserialize_base64string_as_h256, deserialize_from_str,
    deserialize_string_as_bytes, deserialize_timestamp_from_rfc3339, timestamp_from_rfc3339,
};
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use scale_info::{prelude::string::String, TypeInfo};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use serde_json::Value;
use sp_core::{H160, H256, H512};
use sp_std::{borrow::ToOwned, time::Duration, vec::Vec};
#[cfg(feature = "std")]
use subtle_encoding::hex;
use tendermint::{
    block::{self, header::Version, parts::Header as PartSetHeader, Commit, CommitSig, Header},
    chain, hash, node,
    validator::{self, ProposerPriority},
    vote, Time,
};
use tendermint_light_client_verifier::{
    options::Options,
    types::{LightBlock, SignedHeader, TrustThreshold, ValidatorSet},
};

pub type TendermintVoteSignature = H512;
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

impl TrustThresholdStorage {
    pub fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }
}

impl TryFrom<TrustThresholdStorage> for TrustThreshold {
    type Error = tendermint::Error;

    fn try_from(value: TrustThresholdStorage) -> Result<Self, Self::Error> {
        Self::new(value.numerator, value.denominator)
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LightClientOptionsStorage {
    /// Defines what fraction of the total voting power of a known
    /// and trusted validator set is sufficient for a commit to be
    /// accepted going forward.
    pub trust_threshold: TrustThresholdStorage,
    /// How long a validator set is trusted for (must be shorter than the chain's
    /// unbonding period) in secs
    pub trusting_period: u64,
    /// Correction parameter dealing with only approximately synchronized clocks.
    /// The local clock should always be ahead of timestamps from the blockchain; this
    /// is the maximum amount that the local clock may drift behind a timestamp from the
    /// blockchain.
    pub clock_drift: u64,
}

impl LightClientOptionsStorage {
    pub fn new(
        trust_threshold: TrustThresholdStorage,
        trusting_period: u64,
        clock_drift: u64,
    ) -> Self {
        Self {
            trust_threshold,
            trusting_period,
            clock_drift,
        }
    }
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

    fn try_from(value: LightClientOptionsStorage) -> Result<Self, Self::Error> {
        Ok(Self {
            trust_threshold: value
                .trust_threshold
                .try_into()
                .expect("Can't create TrustThreshold"),
            trusting_period: Duration::from_secs(value.trusting_period),
            clock_drift: Duration::from_secs(value.clock_drift),
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct VersionStorage {
    /// Block version    
    #[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_str"))]
    pub block: u64,
    /// App version
    #[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_str"))]
    pub app: u64,
}

impl VersionStorage {
    pub fn new(block: u64, app: u64) -> Self {
        Self { block, app }
    }
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
    pub hash: BridgedBlockHash,
}

impl PartSetHeaderStorage {
    pub fn new(total: u32, hash: BridgedBlockHash) -> Self {
        Self { total, hash }
    }
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

impl BlockIdStorage {
    pub fn new(hash: BridgedBlockHash, part_set_header: PartSetHeaderStorage) -> Self {
        Self {
            hash,
            part_set_header,
        }
    }
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
    #[cfg_attr(
        feature = "std",
        serde(deserialize_with = "deserialize_string_as_bytes")
    )]
    pub chain_id: Vec<u8>,
    /// Current block height
    #[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_str"))]
    pub height: u64,
    /// Epoch Unix timestamp in seconds
    #[cfg_attr(
        feature = "std",
        serde(
            alias = "time",
            alias = "timestamp",
            deserialize_with = "deserialize_timestamp_from_rfc3339"
        )
    )]
    pub timestamp: TimestampStorage,
    /// Previous block info
    pub last_block_id: Option<BlockIdStorage>,
    /// Commit from validators from the last block
    pub last_commit_hash: Option<Hash>,
    /// Merkle root of transaction hashes
    pub data_hash: Option<Hash>,
    /// Validators for the current block
    pub validators_hash: Hash,
    /// Validators for the next block
    pub next_validators_hash: Hash,
    /// Consensus params for the current block
    pub consensus_hash: Hash,
    /// State after txs from the previous block
    /// AppHash is usually a SHA256 hash, but in reality it can be any kind of data
    #[cfg_attr(
        feature = "std",
        serde(deserialize_with = "deserialize_string_as_bytes")
    )]
    pub app_hash: Vec<u8>,
    /// Root hash of all results from the txs from the previous block
    pub last_results_hash: Option<Hash>,
    /// Hash of evidence included in the block
    pub evidence_hash: Option<Hash>,
    /// Original proposer of the block
    pub proposer_address: TendermintAccountId,
}

impl HeaderStorage {
    pub fn new(
        version: VersionStorage,
        chain_id: Vec<u8>,
        height: u64,
        timestamp: TimestampStorage,
        last_block_id: Option<BlockIdStorage>,
        last_commit_hash: Option<Hash>,
        data_hash: Option<Hash>,
        validators_hash: Hash,
        next_validators_hash: Hash,
        consensus_hash: Hash,
        app_hash: Vec<u8>,
        last_results_hash: Option<Hash>,
        evidence_hash: Option<Hash>,
        proposer_address: TendermintAccountId,
    ) -> Self {
        Self {
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
        }
    }
}

/// Represents  UTC time since Unix epoch
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TimestampStorage {
    pub seconds: i64,
    pub nanos: u32,
}

impl TimestampStorage {
    pub fn new(seconds: i64, nanos: u32) -> Self {
        Self { seconds, nanos }
    }
}

impl TryFrom<TimestampStorage> for Time {
    type Error = tendermint::Error;

    fn try_from(value: TimestampStorage) -> Result<Self, Self::Error> {
        Time::from_unix_timestamp(value.seconds, value.nanos)
    }
}

/// CommitSig represents a signature of a validator.
/// It's a part of the Commit and can be used to reconstruct the vote set given the validator set.
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum CommitSignatureStorage {
    /// no vote was received from a validator.
    BlockIdFlagAbsent,
    /// voted for the Commit.BlockID.
    BlockIdFlagCommit {
        /// Validator address
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: TimestampStorage,
        /// Signature of vote
        signature: TendermintVoteSignature,
    },
    /// voted for nil.
    BlockIdFlagNil {
        /// Validator address
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: TimestampStorage,
        /// Signature of vote
        signature: TendermintVoteSignature,
    },
}

impl CommitSignatureStorage {
    fn signature(signature: TendermintVoteSignature) -> Option<tendermint::Signature> {
        tendermint::Signature::try_from(signature.as_bytes()).ok()
    }
}

#[cfg(feature = "std")]
impl<'de> serde::Deserialize<'de> for CommitSignatureStorage {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;

        Ok(
            match value.get("block_id_flag").and_then(Value::as_u64).unwrap() {
                1 => todo!(),
                2 => {
                    let s = value
                        .get("validator_address")
                        .and_then(Value::as_str)
                        .unwrap();
                    let bytes = hex::decode_upper(s).or_else(|_| hex::decode(s)).unwrap();
                    let validator_address = TendermintAccountId::from_slice(&bytes);

                    let timestamp = timestamp_from_rfc3339(
                        value.get("timestamp").and_then(Value::as_str).unwrap(),
                    )
                    .unwrap();

                    let signature = base64string_as_h512(
                        value.get("signature").and_then(Value::as_str).unwrap(),
                    )
                    .unwrap();

                    CommitSignatureStorage::BlockIdFlagCommit {
                        validator_address,
                        timestamp,
                        signature,
                    }
                }
                3 => todo!(),
                other_ => panic!("unsupported block_id_flag {:?}", other_),
            },
        )
    }
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
                let timestamp = timestamp.try_into().unwrap();
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
                let timestamp = timestamp.try_into().unwrap();
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
    #[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_str"))]
    pub height: u64,
    /// Round
    pub round: u32,
    /// Block ID
    pub block_id: BlockIdStorage,
    /// Signatures
    pub signatures: Vec<CommitSignatureStorage>,
}

impl CommitStorage {
    pub fn new(
        height: u64,
        round: u32,
        block_id: BlockIdStorage,
        signatures: Vec<CommitSignatureStorage>,
    ) -> Self {
        Self {
            height,
            round,
            block_id,
            signatures,
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
            chain_id:
            // chain_id
            //     .parse::<chain::Id>()
            //     .expect("Cannot parse as Chain Id"),
            String::from_utf8(chain_id)
                .expect("Not a UTF8 string encoding")
                .parse::<chain::Id>()
                .expect("Cannot parse as Chain Id"),
            height: block::Height::try_from(height).expect("Cannot create Height"),
            time: timestamp.try_into().unwrap(),
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
            app_hash:
            // hash::AppHash::from_hex_upper(&app_hash).expect("Cannot create AppHash"),
            hash::AppHash::try_from(app_hash).expect("Cannot create AppHash"),
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

impl SignedHeaderStorage {
    pub fn new(header: HeaderStorage, commit: CommitStorage) -> Self {
        Self { header, commit }
    }
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
    #[cfg_attr(
        feature = "std",
        serde(
            rename = "tendermint/PubKeyEd25519",
            deserialize_with = "deserialize_base64string_as_h256"
        )
    )]
    Ed25519(H256),
    #[cfg_attr(
        feature = "std",
        serde(
            rename = "tendermint/PubKeySecp256k1",
            deserialize_with = "deserialize_base64string_as_h256"
        )
    )]
    Secp256k1(H256),
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
        serde(
            alias = "voting_power",
            // alias = "total_voting_power",
            deserialize_with = "deserialize_from_str"
        )
    )]
    pub power: u64,
    /// Validator name
    pub name: Option<Vec<u8>>,
    /// Validator proposer priority
    #[cfg_attr(feature = "std", serde(skip))]
    pub proposer_priority: i64,
}

impl ValidatorInfoStorage {
    pub fn new(
        address: TendermintAccountId,
        pub_key: TendermintPublicKey,
        power: u64,
        name: Option<Vec<u8>>,
        proposer_priority: i64,
    ) -> Self {
        Self {
            address,
            pub_key,
            power,
            name,
            proposer_priority,
        }
    }
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
                TendermintPublicKey::Ed25519(hash) => {
                    tendermint::PublicKey::from_raw_ed25519(hash.as_bytes())
                        .expect("Not a ed25519 public key")
                }
                TendermintPublicKey::Secp256k1(hash) => {
                    tendermint::PublicKey::from_raw_secp256k1(&hash.as_bytes())
                        .expect("Not a secp256k1 public key")
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
    #[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_str"))]
    pub total_voting_power: u64,
}

impl ValidatorSetStorage {
    pub fn new(
        validators: Vec<ValidatorInfoStorage>,
        proposer: Option<ValidatorInfoStorage>,
        total_voting_power: u64,
    ) -> Self {
        Self {
            validators,
            proposer,
            total_voting_power,
        }
    }
}

impl TryFrom<ValidatorSetStorage> for ValidatorSet {
    type Error = &'static str;

    fn try_from(value: ValidatorSetStorage) -> Result<Self, Self::Error> {
        let ValidatorSetStorage {
            validators,
            proposer,
            ..
        } = value;

        // NOTE: constructor will sum up voting powers
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

impl LightBlockStorage {
    pub fn new(
        signed_header: SignedHeaderStorage,
        validators: ValidatorSetStorage,
        next_validators: ValidatorSetStorage,
        provider: TendermintPeerId,
    ) -> Self {
        Self {
            signed_header,
            validators,
            next_validators,
            provider,
        }
    }
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
