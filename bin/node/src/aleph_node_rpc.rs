use std::sync::Arc;

use finality_aleph::{AlephJustification, BlockId, Justification, JustificationTranslator};
use futures::channel::mpsc;
use jsonrpsee::{
    core::{error::Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use primitives::{AccountId, AlephSessionApi, Signature};
use sp_api::ProvideRuntimeApi;
use sp_arithmetic::traits::Zero;
use sp_blockchain::HeaderBackend;
use sp_consensus_aura::digests::CompatibleDigestItem;
use sp_runtime::{
    traits::{Block as BlockT, Header as HeaderT},
    DigestItem,
};

use crate::aleph_primitives::BlockNumber;

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
    /// Failed to get session data at the parent block.
    #[error("Failed to get session data at the parent block.")]
    SessionInfoNotAvailable,
    /// Failed to get authority set at the parent block.
    #[error("Failed to get authority set at the parent block.")]
    AuthoritiesInfoNotAvailable,
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
// Failed to get session data at the parent block.
const SESSION_INFO_NOT_AVAILABLE_ERROR: i32 = BASE_ERROR + 5;
// Failed to get authority set at the parent block.
const AUTHORITIES_INFO_NOT_AVAILABLE_ERROR: i32 = BASE_ERROR + 6;

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
            Error::SessionInfoNotAvailable => CallError::Custom(ErrorObject::owned(
                SESSION_INFO_NOT_AVAILABLE_ERROR,
                "Failed to get session data at the parent block.",
                None::<()>,
            )),
            Error::AuthoritiesInfoNotAvailable => CallError::Custom(ErrorObject::owned(
                AUTHORITIES_INFO_NOT_AVAILABLE_ERROR,
                "Failed to get authority set at the parent block.",
                None::<()>,
            )),
        }
        .into()
    }
}

/// Aleph Node RPC API
#[rpc(client, server)]
pub trait AlephNodeApi<Block: BlockT> {
    /// Finalize the block with given hash and number using attached signature. Returns the empty string or an error.
    #[method(name = "alephNode_emergencyFinalize")]
    fn aleph_node_emergency_finalize(
        &self,
        justification: Vec<u8>,
        hash: Block::Hash,
        number: <<Block as BlockT>::Header as HeaderT>::Number,
    ) -> RpcResult<()>;

    /// Get the author of the block with given hash.
    #[method(name = "chain_getBlockAuthor")]
    fn aleph_node_block_author(
        &self,
        hash: Block::Hash,
    ) -> RpcResult<Option<(Vec<AccountId>, Vec<AccountId>)>>;
}

/// Aleph Node API implementation
pub struct AlephNode<Block, JT, Client>
where
    Block: BlockT,
    Block::Header: HeaderT<Number = BlockNumber>,
    JT: JustificationTranslator<Block::Header> + Send + Sync + Clone + 'static,
{
    import_justification_tx: mpsc::UnboundedSender<Justification<Block::Header>>,
    justification_translator: JT,
    client: Arc<Client>,
}

impl<Block, JT, Client> AlephNode<Block, JT, Client>
where
    Block: BlockT,
    Block::Header: HeaderT<Number = BlockNumber>,
    JT: JustificationTranslator<Block::Header> + Send + Sync + Clone + 'static,
    Client: HeaderBackend<Block> + 'static,
{
    pub fn new(
        import_justification_tx: mpsc::UnboundedSender<Justification<Block::Header>>,
        justification_translator: JT,
        client: Arc<Client>,
    ) -> Self {
        AlephNode {
            import_justification_tx,
            justification_translator,
            client,
        }
    }
}

impl<Block, JT, Client> AlephNodeApiServer<Block> for AlephNode<Block, JT, Client>
where
    Block: BlockT,
    Block::Header: HeaderT<Number = BlockNumber>,
    JT: JustificationTranslator<Block::Header> + Send + Sync + Clone + 'static,
    Client: HeaderBackend<Block> + ProvideRuntimeApi<Block> + 'static,
    Client::Api: AlephSessionApi<Block>,
{
    fn aleph_node_emergency_finalize(
        &self,
        justification: Vec<u8>,
        hash: Block::Hash,
        number: <<Block as BlockT>::Header as HeaderT>::Number,
    ) -> RpcResult<()> {
        let justification: AlephJustification =
            AlephJustification::EmergencySignature(justification.try_into().map_err(|_| {
                Error::MalformedJustificationArg(
                    "Provided justification cannot be converted into correct type".into(),
                )
            })?);
        let justification = self
            .justification_translator
            .translate(justification, BlockId::new(hash, number))
            .map_err(|e| Error::FailedJustificationTranslation(format!("{}", e)))?;
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

    fn aleph_node_block_author(
        &self,
        hash: Block::Hash,
    ) -> RpcResult<Option<(Vec<AccountId>, Vec<AccountId>)>> {
        let header = self.client.header(hash).unwrap().unwrap();
        if header.number().is_zero() {
            return Ok(None);
        }

        let parent = header.parent_hash();

        let block_producers_at_parent_pallet_session = self
            .client
            .runtime_api()
            .session_validators(*parent)
            .map_err(|_| Error::AuthoritiesInfoNotAvailable)?;

        let session_index_at_parent = self
            .client
            .runtime_api()
            .current_session(*parent)
            .map_err(|_| Error::SessionInfoNotAvailable)?;
        let block_producers_at_parent_pallet_cm = self
            .client
            .runtime_api()
            .session_committee(*parent, session_index_at_parent)
            .map_err(|_| Error::AuthoritiesInfoNotAvailable)?
            .map_err(|_| Error::AuthoritiesInfoNotAvailable)?
            .block_producers;

        Ok(Some((
            block_producers_at_parent_pallet_session,
            block_producers_at_parent_pallet_cm,
        )))
    }
}
