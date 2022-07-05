use futures::channel::mpsc;
use jsonrpsee::{
	core::{error::Error as JsonRpseeError, RpcResult},
	types::error::{CallError, ErrorObject},
};

use jsonrpsee::{
	proc_macros::rpc,
};

use sc_chain_spec::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Running node's static details.
#[derive(Clone, Debug)]
pub struct SystemInfo {
	/// Implementation name.
	pub impl_name: String,
	/// Implementation version.
	pub impl_version: String,
	/// Chain name.
	pub chain_name: String,
	/// A custom set of properties defined in the chain spec.
	pub properties: Properties,
	/// The type of this chain.
	pub chain_type: ChainType,
}

/// Health struct returned by the RPC
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
	/// Number of connected peers
	pub peers: usize,
	/// Is the node syncing
	pub is_syncing: bool,
	/// Should this node have any peers
	///
	/// Might be false for local chains or when running without discovery.
	pub should_have_peers: bool,
}

impl fmt::Display for Health {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		write!(fmt, "{} peers ({})", self.peers, if self.is_syncing { "syncing" } else { "idle" })
	}
}

/// Network Peer information
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo<Hash, Number> {
	/// Peer ID
	pub peer_id: String,
	/// Roles
	pub roles: String,
	/// Peer best block hash
	pub best_hash: Hash,
	/// Peer best block number
	pub best_number: Number,
}

/// The role the node is running as
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeRole {
	/// The node is a full node
	Full,
	/// The node is a light client
	LightClient,
	/// The node is an authority
	Authority,
}

/// System RPC errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
	/// Provided block range couldn't be resolved to a list of blocks.
	#[error("Node is not fully functional: {}", .0)]
	NotHealthy(Health),
	/// Peer argument is malformatted.
	#[error("{0}")]
	MalformattedPeerArg(String),
}

// Base code for all system errors.
const BASE_ERROR: i32 = 2000;
// Provided block range couldn't be resolved to a list of blocks.
const NOT_HEALTHY_ERROR: i32 = BASE_ERROR + 1;
// Peer argument is malformatted.
const MALFORMATTED_PEER_ARG_ERROR: i32 = BASE_ERROR + 2;

impl From<Error> for JsonRpseeError {
	fn from(e: Error) -> Self {
		match e {
			Error::NotHealthy(ref h) =>
				CallError::Custom(ErrorObject::owned(NOT_HEALTHY_ERROR, e.to_string(), Some(h))),
			Error::MalformattedPeerArg(e) => CallError::Custom(ErrorObject::owned(
				MALFORMATTED_PEER_ARG_ERROR + 2,
				e,
				None::<()>,
			)),
		}
		.into()
	}
}

/// Substrate system RPC API
#[rpc(client, server)]
pub trait AlephNodeApi<Hash, Number> {
	/// Remove a reserved peer. Returns the empty string or an error. The string
	/// should encode only the PeerId e.g. `QmSk5HQbn6LhUwDiNMseVUjuRYhEtYj4aUZ6WfWoGURpdV`.
	#[method(name = "alephNode_emergencyFinalize")]
	fn aleph_node_emergency_finalize(&self, justification: Vec<u8>, hash: Hash, number: Number) -> RpcResult<()>;
}

use finality_aleph::{JustificationNotification};
use sp_api::BlockT;
use finality_aleph::AlephJustification;
use sp_runtime::traits::NumberFor;

/// System API implementation
pub struct AlephNode<B> where
	B: BlockT,
	B::Hash: Serialize + for<'de> serde::Deserialize<'de>,
	NumberFor<B>: Serialize + for<'de> serde::Deserialize<'de> {
	import_justification_tx: mpsc::UnboundedSender<JustificationNotification<B>>,
}

impl<B> AlephNode<B> where
	B: BlockT,
	B::Hash: Serialize + for<'de> serde::Deserialize<'de>,
	NumberFor<B>: Serialize + for<'de> serde::Deserialize<'de> {
    pub fn new(import_justification_tx: mpsc::UnboundedSender<JustificationNotification<B>>) -> Self {
        AlephNode{
			import_justification_tx
		}
    }
}

impl<B> AlephNodeApiServer<B::Hash, NumberFor<B>> for AlephNode<B> where
	B: BlockT,
	B::Hash: Serialize + for<'de> serde::Deserialize<'de>,
	NumberFor<B>: Serialize + for<'de> serde::Deserialize<'de> {
	fn aleph_node_emergency_finalize(&self, justification: Vec<u8>, hash: B::Hash, number: NumberFor<B>) -> RpcResult<()> {
		let justification = AlephJustification::EmergencySignature(justification.try_into().unwrap());
		
		let _ = self.import_justification_tx.unbounded_send(
			JustificationNotification{
				justification,
				hash,
				number,
			}
		);
		Ok(())
	}
}
