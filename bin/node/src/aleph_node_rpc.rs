use std::{collections::HashMap, net::IpAddr, sync::Arc};

use finality_aleph::{
    AlephJustification, BlockId, Justification, JustificationTranslator, ValidatorAddressCache,
    ValidatorAddressingInfo,
};
use futures::channel::mpsc;
use jsonrpsee::{
    core::{async_trait, error::Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use parity_scale_codec::Decode;
use primitives::{AccountId, Block, BlockHash, BlockNumber, Hash, Signature};
use sc_client_api::StorageProvider;
use sc_network::{multiaddr::Protocol, network_state::NetworkState, Multiaddr, NetworkService};
use sp_arithmetic::traits::Zero;
use sp_blockchain::HeaderBackend;
use sp_consensus::SyncOracle;
use sp_consensus_aura::digests::CompatibleDigestItem;
use sp_core::{twox_128, Bytes};
use sp_runtime::{
    traits::{Block as BlockT, Header as HeaderT},
    DigestItem,
};

/// System RPC errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Justification argument is malformed.
    #[error("{0}")]
    MalformedJustificationArg(String),
    /// Provided block range couldn't be resolved to a list of blocks.
    #[error("Node is not fully functional: {}", .0)]
    FailedJustificationSend(String),
    /// Justification argument is malformed.
    #[error("Failed to translate justification into an internal one: {}", .0)]
    FailedJustificationTranslation(String),
    /// Block doesn't have any Aura pre-runtime digest item.
    #[error("Block doesn't have any Aura pre-runtime digest item.")]
    BlockWithoutDigest,
    /// Failed to get storage item.
    #[error("Failed to get storage item {0}/{1} at block {2}.")]
    StorageItemNotAvailable(&'static str, &'static str, String),
    /// Failed to read storage.
    #[error("Failed to read {0}/{1} at the block {2}: {3:?}.")]
    FailedStorageRead(&'static str, &'static str, String, sp_blockchain::Error),
    /// Failed to decode storage item.
    #[error("Failed to decode storage item: {0}/{1} at the block {2}: {3:?}.")]
    FailedStorageDecoding(
        &'static str,
        &'static str,
        String,
        parity_scale_codec::Error,
    ),
    /// Failed to decode header.
    #[error("Failed to decode header of a block {0}: {1:?}.")]
    FailedHeaderDecoding(String, sp_blockchain::Error),
    /// Failed to find a block with provided hash.
    #[error("Failed to find a block with hash {0}.")]
    UnknownHash(String),
}

// Base code for all system errors.
const BASE_ERROR: i32 = 2000;
// Justification argument is malformatted.
const MALFORMATTED_JUSTIFICATION_ARG_ERROR: i32 = BASE_ERROR + 1;
// AlephNodeApiServer is failed to send translated justification.
const FAILED_JUSTIFICATION_SEND_ERROR: i32 = BASE_ERROR + 2;
// AlephNodeApiServer failed to translate justification into internal representation.
const FAILED_JUSTIFICATION_TRANSLATION_ERROR: i32 = BASE_ERROR + 3;
// Block doesn't have any Aura pre-runtime digest item.
const BLOCK_WITHOUT_DIGEST_ERROR: i32 = BASE_ERROR + 4;
// Failed to get storage item.
const STORAGE_ITEM_NOT_AVAILABLE_ERROR: i32 = BASE_ERROR + 5;
/// Failed to read storage.
const FAILED_STORAGE_READ_ERROR: i32 = BASE_ERROR + 6;
/// Failed to decode storage item.
const FAILED_STORAGE_DECODING_ERROR: i32 = BASE_ERROR + 7;
/// Failed to decode header.
const FAILED_HEADER_DECODING_ERROR: i32 = BASE_ERROR + 8;
/// Failed to find a block with provided hash.
const UNKNOWN_HASH_ERROR: i32 = BASE_ERROR + 9;

impl From<Error> for JsonRpseeError {
    fn from(e: Error) -> Self {
        match e {
            Error::FailedJustificationSend(e) => CallError::Custom(ErrorObject::owned(
                FAILED_JUSTIFICATION_SEND_ERROR,
                e,
                None::<()>,
            )),
            Error::MalformedJustificationArg(e) => CallError::Custom(ErrorObject::owned(
                MALFORMATTED_JUSTIFICATION_ARG_ERROR,
                e,
                None::<()>,
            )),
            Error::FailedJustificationTranslation(e) => CallError::Custom(ErrorObject::owned(
                FAILED_JUSTIFICATION_TRANSLATION_ERROR,
                e,
                None::<()>,
            )),
            Error::BlockWithoutDigest => CallError::Custom(ErrorObject::owned(
                BLOCK_WITHOUT_DIGEST_ERROR,
                "Block doesn't have any Aura pre-runtime digest item.",
                None::<()>,
            )),
            Error::StorageItemNotAvailable(pallet, key, hash) => {
                CallError::Custom(ErrorObject::owned(
                    STORAGE_ITEM_NOT_AVAILABLE_ERROR,
                    format!("Failed to get storage item {pallet}/{key} at the block {hash}."),
                    None::<()>,
                ))
            }
            Error::FailedStorageRead(pallet, key, hash, err) => {
                CallError::Custom(ErrorObject::owned(
                    FAILED_STORAGE_READ_ERROR,
                    format!("Failed to read {pallet}/{key} at the block {hash}: {err:?}."),
                    None::<()>,
                ))
            }
            Error::FailedStorageDecoding(pallet, key, hash, err) => {
                CallError::Custom(ErrorObject::owned(
                    FAILED_STORAGE_DECODING_ERROR,
                    format!("Failed to decode {pallet}/{key} at the block {hash}: {err:?}.",),
                    None::<()>,
                ))
            }
            Error::FailedHeaderDecoding(hash, err) => CallError::Custom(ErrorObject::owned(
                FAILED_HEADER_DECODING_ERROR,
                format!("Failed to decode header of a block {hash}: {err:?}.",),
                None::<()>,
            )),
            Error::UnknownHash(hash) => CallError::Custom(ErrorObject::owned(
                UNKNOWN_HASH_ERROR,
                format!("Failed to find a block with hash {hash}.",),
                None::<()>,
            )),
        }
        .into()
    }
}

/// Aleph Node RPC API
#[rpc(client, server, namespace = "alephNode")]
pub trait AlephNodeApi<BE> {
    /// Finalize the block with given hash and number using attached signature. Returns the empty string or an error.
    #[method(name = "emergencyFinalize")]
    fn emergency_finalize(
        &self,
        justification: Bytes,
        hash: BlockHash,
        number: BlockNumber,
    ) -> RpcResult<()>;

    /// Get the author of the block with given hash.
    #[method(name = "getBlockAuthor")]
    fn block_author(&self, hash: BlockHash) -> RpcResult<Option<AccountId>>;

    ///
    #[method(name = "ready")]
    fn ready(&self) -> RpcResult<bool>;

    #[method(name = "validatorNetworkInfo")]
    async fn validator_network_info(
        &self,
    ) -> RpcResult<HashMap<AccountId, ValidatorAddressingInfo>>;
}

/// Aleph Node API implementation
pub struct AlephNode<Client, SO> {
    import_justification_tx: mpsc::UnboundedSender<Justification>,
    justification_translator: JustificationTranslator,
    client: Arc<Client>,
    sync_oracle: SO,
    validator_address_cache: ValidatorAddressCache,
    network: Arc<NetworkService<Block, Hash>>,
}

impl<Client, SO> AlephNode<Client, SO>
where
    SO: SyncOracle,
{
    pub fn new(
        import_justification_tx: mpsc::UnboundedSender<Justification>,
        justification_translator: JustificationTranslator,
        client: Arc<Client>,
        sync_oracle: SO,
        validator_address_cache: ValidatorAddressCache,
        network: Arc<NetworkService<Block, Hash>>,
    ) -> Self {
        AlephNode {
            import_justification_tx,
            justification_translator,
            client,
            sync_oracle,
            validator_address_cache,
            network,
        }
    }
}

#[async_trait]
impl<Client, BE, SO> AlephNodeApiServer<BE> for AlephNode<Client, SO>
where
    BE: sc_client_api::Backend<Block> + 'static,
    Client: HeaderBackend<Block> + StorageProvider<Block, BE> + 'static,
    SO: SyncOracle + Send + Sync + 'static,
{
    fn emergency_finalize(
        &self,
        justification: Bytes,
        hash: BlockHash,
        number: BlockNumber,
    ) -> RpcResult<()> {
        let justification: AlephJustification =
            AlephJustification::EmergencySignature(justification.0.try_into().map_err(|_| {
                Error::MalformedJustificationArg(
                    "Provided justification cannot be converted into correct type".into(),
                )
            })?);
        let justification = self
            .justification_translator
            .translate(justification, BlockId::new(hash, number))
            .map_err(|e| Error::FailedJustificationTranslation(format!("{e}")))?;
        self.import_justification_tx
            .unbounded_send(justification)
            .map_err(|_| {
                Error::FailedJustificationSend(
                    "AlephNodeApiServer failed to send JustifictionNotification via its channel"
                        .into(),
                )
            })?;
        Ok(())
    }

    fn block_author(&self, hash: BlockHash) -> RpcResult<Option<AccountId>> {
        let header = self
            .client
            .header(hash)
            .map_err(|e| Error::FailedHeaderDecoding(hash.to_string(), e))?
            .ok_or(Error::UnknownHash(hash.to_string()))?;
        if header.number().is_zero() {
            return Ok(None);
        }

        let slot = header
            .digest()
            .logs()
            .iter()
            .find_map(<DigestItem as CompatibleDigestItem<Signature>>::as_aura_pre_digest)
            .ok_or(Error::BlockWithoutDigest)?;

        let parent = header.parent_hash();
        let block_producers_at_parent: Vec<AccountId> =
            read_storage("Session", "Validators", &self.client, *parent)?;

        Ok(Some(
            block_producers_at_parent[(u64::from(slot) as usize) % block_producers_at_parent.len()]
                .clone(),
        ))
    }

    fn ready(&self) -> RpcResult<bool> {
        Ok(!self.sync_oracle.is_offline() && !self.sync_oracle.is_major_syncing())
    }

    async fn validator_network_info(
        &self,
    ) -> RpcResult<HashMap<AccountId, ValidatorAddressingInfo>> {
        let mut info = self.validator_address_cache.read();

        // This uses unstable method from substrate's API, but there's probably no other easy way
        // of doing this. On the other hand, the p2p peer_id is only for debuging purposes,
        // so in case of future substrate API change this if statement can be temporarily safely removed.
        if let Ok(network_state) = self.network.network_state().await {
            add_p2p_peer_id_to_validator_addressing_info(&mut info, network_state);
        }
        Ok(info)
    }
}

fn add_p2p_peer_id_to_validator_addressing_info(
    info: &mut HashMap<AccountId, ValidatorAddressingInfo>,
    network_state: NetworkState,
) {
    let mut ip_to_peer_id = HashMap::new();
    network_state
        .connected_peers
        .iter()
        .flat_map(|(peer_id, peer)| peer.known_addresses.iter().map(move |addr| (addr, peer_id)))
        .for_each(|(addr, peer_id)| {
            if let Some(ip_address) = try_to_ip_addr(addr) {
                ip_to_peer_id
                    .entry(ip_address)
                    .or_insert(vec![])
                    .push(peer_id.clone());
            }
        });
    for (_, info) in info.iter_mut() {
        if let Ok(addr) = info.network_level_address.parse::<IpAddr>() {
            if let Some(peer_ids) = ip_to_peer_id.get(&addr) {
                info.potential_p2p_network_peer_ids = peer_ids.clone();
            }
        }
    }
}

fn try_to_ip_addr(multiaddr: &Multiaddr) -> Option<IpAddr> {
    for component in multiaddr.iter() {
        if let Protocol::Ip4(addr) = component {
            return Some(IpAddr::V4(addr));
        } else if let Protocol::Ip6(addr) = component {
            return Some(IpAddr::V6(addr));
        }
    }
    None
}

fn read_storage<
    T: Decode,
    Block: BlockT,
    Backend: sc_client_api::Backend<Block>,
    SP: StorageProvider<Block, Backend>,
>(
    pallet: &'static str,
    pallet_item: &'static str,
    storage_provider: &Arc<SP>,
    block_hash: Block::Hash,
) -> RpcResult<T> {
    let storage_key = [
        twox_128(pallet.as_bytes()),
        twox_128(pallet_item.as_bytes()),
    ]
    .concat();

    let item_encoded = match storage_provider
        .storage(block_hash, &sc_client_api::StorageKey(storage_key))
    {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return Err(
                Error::StorageItemNotAvailable(pallet, pallet_item, block_hash.to_string()).into(),
            )
        }
        Err(e) => {
            return Err(
                Error::FailedStorageRead(pallet, pallet_item, block_hash.to_string(), e).into(),
            )
        }
    };

    T::decode(&mut item_encoded.0.as_ref()).map_err(|e| {
        Error::FailedStorageDecoding(pallet, pallet_item, block_hash.to_string(), e).into()
    })
}
