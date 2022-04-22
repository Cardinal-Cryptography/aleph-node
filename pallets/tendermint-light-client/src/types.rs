use crate::utils::{
    account_id_from_bytes, as_tendermint_signature, sha256_from_bytes, tendermint_hash_to_h256,
};
#[cfg(feature = "std")]
use crate::utils::{
    base64string_as_h512, deserialize_base64string_as_h256, deserialize_from_str,
    deserialize_string_as_bytes, deserialize_timestamp_from_rfc3339, timestamp_from_rfc3339,
};
use codec::{Decode, Encode};
use scale_info::{prelude::string::String, TypeInfo};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize};
#[cfg(feature = "std")]
use serde_json::Value;
use sp_core::{RuntimeDebug, H160, H256, H512};
use sp_std::{borrow::ToOwned, prelude::*, time::Duration, vec::Vec};
#[cfg(feature = "std")]
use subtle_encoding::hex;
use tendermint::{
    block::{self, header::Version, parts::Header as PartSetHeader, Commit, CommitSig, Header},
    chain, hash, node,
    validator::{self, ProposerPriority},
    vote, Hash as TendermintHash, PublicKey as TmPublicKey, Time,
};
use tendermint_light_client_verifier::{
    options::Options,
    types::{LightBlock, SignedHeader, TrustThreshold, ValidatorSet},
};
use tendermint_proto::google::protobuf::Timestamp as TmTimestamp;

pub type TendermintVoteSignature = H512;
pub type TendermintPeerId = H160;
pub type TendermintAccountId = H160;
pub type TendermintBlockHash = H256;

#[derive(RuntimeDebug)]
pub enum ConversionError {
    /// error with a context in a str
    WithContext(&'static str),
    /// wrapper for tendermint error types
    Upstream(tendermint::Error),
}

impl From<tendermint::Error> for ConversionError {
    fn from(e: tendermint::Error) -> Self {
        Self::Upstream(e)
    }
}

#[derive(Encode, Decode, Clone, Copy, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum TendermintHashStorage {
    /// SHA-256 hash
    Some(TendermintBlockHash),
    /// empty hash
    None,
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for TendermintHashStorage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode_upper(&s).or_else(|_| hex::decode(s)).unwrap();
        Ok(Self::Some(H256::from_slice(&bytes)))
    }
}

impl TryFrom<tendermint::Hash> for TendermintHashStorage {
    type Error = ConversionError;

    fn try_from(value: tendermint::Hash) -> Result<Self, Self::Error> {
        match value {
            TendermintHash::Sha256(bytes) => {
                if bytes.len() == hash::SHA256_HASH_SIZE {
                    Ok(TendermintHashStorage::Some(H256::from_slice(&bytes)))
                } else {
                    Err(ConversionError::WithContext("Invalid hash size"))
                }
            }
            TendermintHash::None => Ok(TendermintHashStorage::None),
        }
    }
}

impl From<TendermintHashStorage> for TendermintHash {
    fn from(value: TendermintHashStorage) -> Self {
        match value {
            TendermintHashStorage::Some(hash) => TendermintHash::Sha256(hash.0),
            TendermintHashStorage::None => TendermintHash::None,
        }
    }
}

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
    type Error = ConversionError;

    fn try_from(value: TrustThresholdStorage) -> Result<Self, Self::Error> {
        Self::new(value.numerator, value.denominator).map_err(ConversionError::Upstream)
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

#[cfg(test)]
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
    type Error = ConversionError;

    fn try_from(value: LightClientOptionsStorage) -> Result<Self, Self::Error> {
        let trust_threshold = value.trust_threshold.try_into()?;

        Ok(Self {
            trust_threshold,
            trusting_period: Duration::from_secs(value.trusting_period),
            clock_drift: Duration::from_secs(value.clock_drift),
        })
    }
}

impl From<Options> for LightClientOptionsStorage {
    fn from(value: Options) -> Self {
        let trust_threshold = TrustThresholdStorage::new(
            value.trust_threshold.numerator(),
            value.trust_threshold.denominator(),
        );
        Self::new(
            trust_threshold,
            value.trusting_period.as_secs(),
            value.clock_drift.as_secs(),
        )
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

impl From<Version> for VersionStorage {
    fn from(version: Version) -> Self {
        VersionStorage {
            block: version.block,
            app: version.app,
        }
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PartSetHeaderStorage {
    /// Number of parts in this block
    pub total: u32,
    /// SHA256 Hash of the parts set header,
    pub hash: TendermintHashStorage,
}

impl PartSetHeaderStorage {
    pub fn new(total: u32, hash: TendermintHashStorage) -> Self {
        Self { total, hash }
    }
}

impl TryFrom<PartSetHeaderStorage> for PartSetHeader {
    type Error = ConversionError;

    fn try_from(value: PartSetHeaderStorage) -> Result<Self, Self::Error> {
        let PartSetHeaderStorage { total, hash } = value;
        Ok(Self::new(total, hash.into())?)
    }
}

impl TryFrom<PartSetHeader> for PartSetHeaderStorage {
    type Error = ConversionError;

    fn try_from(value: PartSetHeader) -> Result<Self, Self::Error> {
        let PartSetHeader { total, hash, .. } = value;
        Ok(Self {
            total,
            hash: hash.try_into()?,
        })
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BlockIdStorage {
    /// The block's main hash is the Merkle root of all the fields in the
    /// block header.
    pub hash: TendermintHashStorage,
    /// Parts header (if available) is used for secure gossipping of the block
    /// during consensus. It is the Merkle root of the complete serialized block
    /// cut into parts.
    pub part_set_header: PartSetHeaderStorage,
}

impl BlockIdStorage {
    pub fn new(hash: TendermintHashStorage, part_set_header: PartSetHeaderStorage) -> Self {
        Self {
            hash,
            part_set_header,
        }
    }
}

impl TryFrom<BlockIdStorage> for block::Id {
    type Error = ConversionError;

    fn try_from(value: BlockIdStorage) -> Result<Self, Self::Error> {
        let BlockIdStorage {
            hash,
            part_set_header,
        } = value;

        Ok(Self {
            hash: hash.into(),
            part_set_header: part_set_header.try_into()?,
        })
    }
}

impl TryFrom<block::Id> for BlockIdStorage {
    type Error = ConversionError;

    fn try_from(value: block::Id) -> Result<Self, Self::Error> {
        let block::Id {
            hash,
            part_set_header,
        } = value;

        Ok(Self {
            hash: hash.try_into()?,
            part_set_header: part_set_header.try_into()?,
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
    pub last_commit_hash: Option<H256>,
    /// Merkle root of transaction hashes
    pub data_hash: Option<H256>,
    /// Validators for the current block
    pub validators_hash: H256,
    /// Validators for the next block
    pub next_validators_hash: H256,
    /// Consensus params for the current block
    pub consensus_hash: H256,
    /// State after txs from the previous block
    /// AppHash is usually a SHA256 hash, but in reality it can be any kind of data
    #[cfg_attr(
        feature = "std",
        serde(deserialize_with = "deserialize_string_as_bytes")
    )]
    pub app_hash: Vec<u8>,
    /// Root hash of all results from the txs from the previous block
    pub last_results_hash: Option<H256>,
    /// Hash of evidence included in the block
    pub evidence_hash: Option<H256>,
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
        last_commit_hash: Option<H256>,
        data_hash: Option<H256>,
        validators_hash: H256,
        next_validators_hash: H256,
        consensus_hash: H256,
        app_hash: Vec<u8>,
        last_results_hash: Option<H256>,
        evidence_hash: Option<H256>,
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
    type Error = ConversionError;

    fn try_from(value: TimestampStorage) -> Result<Self, Self::Error> {
        Time::from_unix_timestamp(value.seconds, value.nanos).map_err(ConversionError::Upstream)
    }
}

impl TryFrom<Time> for TimestampStorage {
    type Error = ConversionError;

    fn try_from(value: Time) -> Result<Self, Self::Error> {
        let tm_timestamp: TmTimestamp = value.into();
        if let Ok(nanos) = tm_timestamp.nanos.try_into() {
            Ok(Self {
                seconds: tm_timestamp.seconds,
                nanos,
            })
        } else {
            Err(ConversionError::WithContext("timestamp nanos out of range"))
        }
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
        signature: Option<TendermintVoteSignature>,
    },
    /// voted for nil.
    BlockIdFlagNil {
        /// Validator address
        validator_address: TendermintAccountId,
        /// Timestamp of vote
        timestamp: TimestampStorage,
        /// Signature of vote
        signature: Option<TendermintVoteSignature>,
    },
}

#[cfg(feature = "std")]
impl<'de> serde::Deserialize<'de> for CommitSignatureStorage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;

        Ok(
            match value.get("block_id_flag").and_then(Value::as_u64).unwrap() {
                1 => CommitSignatureStorage::BlockIdFlagAbsent,
                id @ 2 | id @ 3 => {
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

                    let signature = value
                        .get("signature")
                        .and_then(Value::as_str)
                        .map(|sig| base64string_as_h512(sig).unwrap());

                    match id {
                        2 => CommitSignatureStorage::BlockIdFlagCommit {
                            validator_address,
                            timestamp,
                            signature,
                        },
                        3 => CommitSignatureStorage::BlockIdFlagNil {
                            validator_address,
                            timestamp,
                            signature,
                        },
                        _ => panic!("Should never get here"),
                    }
                }

                other_ => panic!("unsupported block_id_flag {:?}", other_),
            },
        )
    }
}

impl TryFrom<CommitSignatureStorage> for CommitSig {
    type Error = ConversionError;

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
                let timestamp = timestamp.try_into()?;
                let signature = match signature {
                    Some(value) => {
                        Some(as_tendermint_signature(value).map_err(ConversionError::Upstream)?)
                    }
                    None => None,
                };

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
                let timestamp = timestamp.try_into()?;
                let signature = match signature {
                    Some(value) => {
                        Some(as_tendermint_signature(value).map_err(ConversionError::Upstream)?)
                    }
                    None => None,
                };

                Self::BlockIdFlagNil {
                    validator_address,
                    timestamp,
                    signature,
                }
            }
        })
    }
}

impl TryFrom<CommitSig> for CommitSignatureStorage {
    type Error = ConversionError;

    fn try_from(commit_sig: CommitSig) -> Result<Self, Self::Error> {
        Ok(match commit_sig {
            CommitSig::BlockIdFlagAbsent => CommitSignatureStorage::BlockIdFlagAbsent,

            CommitSig::BlockIdFlagCommit {
                validator_address,
                timestamp,
                signature,
            } => CommitSignatureStorage::BlockIdFlagCommit {
                validator_address: H160::from_slice(validator_address.as_bytes()),
                timestamp: timestamp.try_into()?,
                signature: signature.map(|sig| H512::from_slice(sig.as_bytes())),
            },

            CommitSig::BlockIdFlagNil {
                validator_address,
                timestamp,
                signature,
            } => CommitSignatureStorage::BlockIdFlagNil {
                validator_address: H160::from_slice(validator_address.as_bytes()),
                timestamp: timestamp.try_into()?,
                signature: signature.map(|sig| H512::from_slice(sig.as_bytes())),
            },
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
    type Error = ConversionError;

    fn try_from(value: CommitStorage) -> Result<Self, Self::Error> {
        let CommitStorage {
            height,
            round,
            block_id,
            signatures,
        } = value;

        let signatures = signatures.into_iter().try_fold(
            Vec::new(),
            |mut acc, element| -> Result<Vec<tendermint::block::CommitSig>, ConversionError> {
                acc.push(element.try_into()?);
                Ok(acc)
            },
        )?;

        Ok(Self {
            height: block::Height::try_from(height)?,
            round: block::Round::try_from(round)?,
            block_id: block_id.try_into()?,
            signatures,
        })
    }
}

impl TryFrom<Commit> for CommitStorage {
    type Error = ConversionError;

    fn try_from(commit: Commit) -> Result<Self, Self::Error> {
        let mut signatures = Vec::with_capacity(commit.signatures.len());
        for sig in commit.signatures {
            signatures.push(sig.try_into()?)
        }
        let block_id = commit.block_id.try_into()?;
        Ok(CommitStorage::new(
            commit.height.value(),
            commit.round.value(),
            block_id,
            signatures,
        ))
    }
}

impl TryFrom<HeaderStorage> for Header {
    type Error = ConversionError;

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
            version: version.into(),
            chain_id: String::from_utf8(chain_id)
                .map_err(|_| ConversionError::WithContext("Not a UTF8 string encoding"))?
                .parse::<chain::Id>()?,
            height: block::Height::try_from(height)?,
            time: timestamp.try_into()?,
            last_block_id: match last_block_id {
                Some(id) => id.try_into().ok(),
                None => None,
            },
            last_commit_hash: match last_commit_hash {
                Some(hash) => {
                    Some(sha256_from_bytes(hash.as_bytes()).map_err(ConversionError::Upstream)?)
                }
                None => None,
            },
            data_hash: match data_hash {
                Some(hash) => {
                    Some(sha256_from_bytes(hash.as_bytes()).map_err(ConversionError::Upstream)?)
                }
                None => None,
            },
            validators_hash: sha256_from_bytes(validators_hash.as_bytes())?,
            next_validators_hash: sha256_from_bytes(next_validators_hash.as_bytes())?,
            consensus_hash: sha256_from_bytes(consensus_hash.as_bytes())?,
            app_hash: hash::AppHash::try_from(app_hash)?,
            last_results_hash: match last_results_hash {
                Some(hash) => {
                    Some(sha256_from_bytes(hash.as_bytes()).map_err(ConversionError::Upstream)?)
                }
                None => None,
            },
            evidence_hash: match evidence_hash {
                Some(hash) => {
                    Some(sha256_from_bytes(hash.as_bytes()).map_err(ConversionError::Upstream)?)
                }
                None => None,
            },
            proposer_address: account_id_from_bytes(proposer_address.as_fixed_bytes().to_owned()),
        })
    }
}

impl TryFrom<Header> for HeaderStorage {
    type Error = ConversionError;

    fn try_from(header: Header) -> Result<Self, Self::Error> {
        let last_commit_hash = header
            .last_commit_hash
            .as_ref()
            .and_then(tendermint_hash_to_h256);

        let data_hash = header.data_hash.as_ref().and_then(tendermint_hash_to_h256);

        let validators_hash = match header.validators_hash {
            TendermintHash::Sha256(secp) => H256::from_slice(&secp),
            TendermintHash::None => {
                return Err(ConversionError::WithContext(
                    "unexpected hash variant for validators_hash field",
                ))
            }
        };
        let next_validators_hash = match header.validators_hash {
            TendermintHash::Sha256(secp) => H256::from_slice(&secp),
            TendermintHash::None => {
                return Err(ConversionError::WithContext(
                    "unexpected hash variant for next_validators_hash field",
                ))
            }
        };
        let consensus_hash = match header.validators_hash {
            TendermintHash::Sha256(secp) => H256::from_slice(&secp),
            TendermintHash::None => {
                return Err(ConversionError::WithContext(
                    "unexpected hash variant for consensus_hash field",
                ))
            }
        };
        let app_hash = header.app_hash.value();
        let last_results_hash = header
            .last_results_hash
            .as_ref()
            .and_then(tendermint_hash_to_h256);
        let evidence_hash = header
            .evidence_hash
            .as_ref()
            .and_then(tendermint_hash_to_h256);
        let proposer_address = TendermintPeerId::from_slice(header.proposer_address.as_bytes());
        let version = header.version.into();
        let chain_id = header.chain_id.as_bytes().to_vec();
        let height = header.height.value();
        let time = header.time.try_into()?;

        let last_block_id = match header.last_block_id {
            Some(last_block_id) => Some(last_block_id.try_into()?),
            None => None,
        };

        Ok(HeaderStorage::new(
            version,
            chain_id,
            height,
            time,
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
        ))
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
    type Error = ConversionError;

    fn try_from(value: SignedHeaderStorage) -> Result<Self, Self::Error> {
        let SignedHeaderStorage { header, commit } = value;
        Ok(Self::new(header.try_into()?, commit.try_into()?)?)
    }
}

impl TryFrom<SignedHeader> for SignedHeaderStorage {
    type Error = ConversionError;

    fn try_from(signed_header: SignedHeader) -> Result<Self, Self::Error> {
        let header = signed_header.header().clone().try_into()?;
        let commit = signed_header.commit().clone().try_into()?;
        Ok(SignedHeaderStorage::new(header, commit))
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
        serde(alias = "voting_power", deserialize_with = "deserialize_from_str")
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
    type Error = ConversionError;

    fn try_from(value: ValidatorInfoStorage) -> Result<Self, Self::Error> {
        let ValidatorInfoStorage {
            address,
            pub_key,
            power,
            name,
            proposer_priority,
            ..
        } = value;

        Ok(Self {
            address: account_id_from_bytes(address.as_fixed_bytes().to_owned()),
            pub_key: match pub_key {
                TendermintPublicKey::Ed25519(hash) => {
                    tendermint::PublicKey::from_raw_ed25519(hash.as_bytes())
                        .ok_or(ConversionError::WithContext("Not a ed25519 public key"))?
                }
                TendermintPublicKey::Secp256k1(hash) => {
                    tendermint::PublicKey::from_raw_secp256k1(hash.as_bytes())
                        .ok_or(ConversionError::WithContext("Not a secp256k1 public key"))?
                }
            },
            power: vote::Power::try_from(power)?,
            name: match name {
                Some(name) => String::from_utf8(name).ok(),
                None => None,
            },
            proposer_priority: ProposerPriority::from(proposer_priority),
        })
    }
}

impl From<TmPublicKey> for TendermintPublicKey {
    fn from(key: TmPublicKey) -> Self {
        match key {
            TmPublicKey::Ed25519(ed) => {
                TendermintPublicKey::Ed25519(H256::from_slice(ed.as_bytes()))
            }
            TmPublicKey::Secp256k1(secp) => {
                TendermintPublicKey::Secp256k1(H256::from_slice(&secp.to_bytes().to_vec()))
            }
            _ => unreachable!(),
        }
    }
}

impl From<validator::Info> for ValidatorInfoStorage {
    fn from(info: validator::Info) -> Self {
        let address = H160::from_slice(info.address.as_bytes());
        let pub_key = info.pub_key.into();
        Self::new(
            address,
            pub_key,
            info.power.value(),
            info.name.clone().map(|s| s.as_bytes().to_vec()),
            info.proposer_priority.value(),
        )
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
    type Error = ConversionError;

    fn try_from(value: ValidatorSetStorage) -> Result<Self, Self::Error> {
        let ValidatorSetStorage {
            validators,
            proposer,
            ..
        } = value;

        let validators = validators.into_iter().try_fold(
            Vec::new(),
            |mut acc, element| -> Result<Vec<validator::Info>, ConversionError> {
                acc.push(element.try_into()?);
                Ok(acc)
            },
        )?;

        // NOTE: constructor will sum up voting powers
        Ok(Self::new(
            validators,
            match proposer {
                Some(proposer) => proposer.try_into().ok(),
                None => None,
            },
        ))
    }
}

impl From<ValidatorSet> for ValidatorSetStorage {
    fn from(value: ValidatorSet) -> Self {
        let validators = value.validators().iter().cloned().map(Into::into).collect();
        let proposer = value.proposer().clone().map(Into::into);
        let total_voting_power = value.total_voting_power().value();
        ValidatorSetStorage::new(validators, proposer, total_voting_power)
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
    type Error = ConversionError;

    fn try_from(value: LightBlockStorage) -> Result<Self, Self::Error> {
        let LightBlockStorage {
            signed_header,
            validators,
            next_validators,
            provider,
        } = value;

        let bytes: [u8; 20] = provider
            .try_into()
            .map_err(|_| ConversionError::WithContext("Not a 20 byte array"))?;

        Ok(Self {
            signed_header: signed_header.try_into()?,
            validators: validators.try_into()?,
            next_validators: next_validators.try_into()?,
            provider: node::Id::new(bytes),
        })
    }
}

impl TryFrom<LightBlock> for LightBlockStorage {
    type Error = ConversionError;

    fn try_from(value: LightBlock) -> Result<Self, Self::Error> {
        let LightBlock {
            signed_header,
            validators,
            next_validators,
            provider,
        } = value;

        Ok(Self {
            signed_header: signed_header.try_into()?,
            validators: validators.into(),
            next_validators: next_validators.into(),
            provider: TendermintPeerId::from_slice(provider.as_bytes()),
        })
    }
}
