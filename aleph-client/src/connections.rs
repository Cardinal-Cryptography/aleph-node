use std::{thread::sleep, time::Duration};

use anyhow::anyhow;
use codec::Decode;
use log::info;
use subxt::{
    ext::sp_core::Bytes,
    metadata::DecodeWithMetadata,
    rpc::RpcParams,
    storage::{address::Yes, StaticStorageAddress, StorageAddress},
    tx::{BaseExtrinsicParamsBuilder, PlainTip, TxPayload},
    SubstrateConfig,
};

use crate::{api, sp_weights::weight_v2::Weight, BlockHash, Call, Client, KeyPair, TxStatus};

/// Capable of communicating with a live Aleph chain.
#[derive(Clone)]
pub struct Connection {
    /// A `subxt` object representing a communication channel to a live chain.
    /// Requires an url address for creation.
    pub client: Client,
}

/// Any connection that is signed by some key.
pub struct SignedConnection {
    /// Composition of [`Connection`] object.
    pub connection: Connection,
    /// A key which signs any txes send via this connection.
    pub signer: KeyPair,
}

/// Specific connection that is singed by the sudo key.
pub struct RootConnection {
    /// Composition of [`Connection`] object.
    pub connection: Connection,
    /// Sudo key pair.
    pub root: KeyPair,
}

/// API for [sudo pallet](https://paritytech.github.io/substrate/master/pallet_sudo/index.html).
#[async_trait::async_trait]
pub trait SudoCall {
    /// API for [`sudo_unchecked_weight`](https://paritytech.github.io/substrate/master/pallet_sudo/pallet/enum.Call.html#variant.sudo_unchecked_weight) call.
    async fn sudo_unchecked(&self, call: Call, status: TxStatus) -> anyhow::Result<BlockHash>;
    /// API for [`sudo`](https://paritytech.github.io/substrate/master/pallet_sudo/pallet/enum.Call.html#variant.sudo) call.
    async fn sudo(&self, call: Call, status: TxStatus) -> anyhow::Result<BlockHash>;
}

#[async_trait::async_trait]
impl SudoCall for RootConnection {
    async fn sudo_unchecked(&self, call: Call, status: TxStatus) -> anyhow::Result<BlockHash> {
        info!(target: "aleph-client", "sending call as sudo_unchecked {:?}", call);
        let sudo = api::tx().sudo().sudo_unchecked_weight(
            call,
            Weight {
                ref_time: 0,
                proof_size: 0,
            },
        );

        self.as_signed().send_tx(sudo, status).await
    }

    async fn sudo(&self, call: Call, status: TxStatus) -> anyhow::Result<BlockHash> {
        info!(target: "aleph-client", "sending call as sudo {:?}", call);
        let sudo = api::tx().sudo().sudo(call);

        self.as_signed().send_tx(sudo, status).await
    }
}

impl Connection {
    const DEFAULT_RETRIES: u32 = 10;
    const RETRY_WAIT_SECS: u64 = 1;

    /// Creates new connection from a given url.
    /// By default, it tries to connect 10 times, waiting 1 second between each unsuccessful attempt.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    pub async fn new(address: String) -> Self {
        Self::new_with_retries(address, Self::DEFAULT_RETRIES).await
    }

    /// Creates new connection from a given url and given number of connection attempts.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    /// * `retries` - number of connection attempts
    pub async fn new_with_retries(address: String, mut retries: u32) -> Self {
        loop {
            let client = Client::from_url(&address).await;
            match (retries, client) {
                (_, Ok(client)) => return Self { client },
                (0, Err(e)) => panic!("{:?}", e),
                _ => {
                    sleep(Duration::from_secs(Self::RETRY_WAIT_SECS));
                    retries -= 1;
                }
            }
        }
    }

    /// Retrieves a decoded storage value stored under given storage key.
    ///
    /// # Panic
    /// This method `panic`s, in case storage key is invalid, or in case value cannot be decoded,
    /// or there is no such value
    /// * `addrs` - represents a storage key, see [more info about keys](https://docs.substrate.io/fundamentals/state-transitions-and-storage/#querying-storage)
    /// * `at` - optional block hash to query state from
    pub async fn get_storage_entry<T: DecodeWithMetadata, Defaultable, Iterable>(
        &self,
        addrs: &StaticStorageAddress<T, Yes, Defaultable, Iterable>,
        at: Option<BlockHash>,
    ) -> T::Target {
        self.get_storage_entry_maybe(addrs, at)
            .await
            .expect("There should be a value")
    }

    /// Retrieves a decoded storage value stored under given storage key.
    ///
    /// # Panic
    /// This method `panic`s, in case storage key is invalid, or in case value cannot be decoded,
    /// but does _not_ `panic` if there is no such value
    /// * `addrs` - represents a storage key, see [more info about keys](https://docs.substrate.io/fundamentals/state-transitions-and-storage/#querying-storage)
    /// * `at` - optional block hash to query state from
    ///
    /// # Examples
    /// ```rust
    ///     let addrs = api::storage().treasury().proposal_count();
    ///     get_storage_entry_maybe(&addrs, None).await
    /// ```
    pub async fn get_storage_entry_maybe<T: DecodeWithMetadata, Defaultable, Iterable>(
        &self,
        addrs: &StaticStorageAddress<T, Yes, Defaultable, Iterable>,
        at: Option<BlockHash>,
    ) -> Option<T::Target> {
        info!(target: "aleph-client", "accessing storage at {}::{} at block {:?}", addrs.pallet_name(), addrs.entry_name(), at);
        self.client
            .storage()
            .fetch(addrs, at)
            .await
            .expect("Should access storage")
    }

    /// Submit a RPC call.
    ///
    /// * `func_name` - name of a RPC call
    /// * `params` - result of calling `rpc_params!` macro, that's `Vec<u8>` of encoded data
    /// to this rpc call
    ///
    /// # Examples
    /// ```rust
    /// let func_name = "alephNode_emergencyFinalize";
    /// let hash = BlockHash::from_str("0x37841c5a09db7d9f985f2306866f196365f1bb9372efc76086e07b882296e1cc").expect("Hash is properly hex encoded");
    /// let signature = key_pair.sign(&hash.encode());
    /// let raw_signature: &[u8] = signature.as_ref();
    /// let params = rpc_params![raw_signature, hash, number];
    /// let _: () = rpc_call(func_name.to_string(), params).await?;
    /// ```
    pub async fn rpc_call<R: Decode>(
        &self,
        func_name: String,
        params: RpcParams,
    ) -> anyhow::Result<R> {
        info!(target: "aleph-client", "submitting rpc call `{}`, with params {:?}", func_name, params);
        let bytes: Bytes = self.client.rpc().request(&func_name, params).await?;

        Ok(R::decode(&mut bytes.as_ref())?)
    }
}

impl SignedConnection {
    /// Creates new signed connection from a given url.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    /// * `signer` - a [`KeyPair`] of signing account
    pub async fn new(address: String, signer: KeyPair) -> Self {
        Self::from_connection(Connection::new(address).await, signer)
    }

    /// Creates new signed connection from existing [`Connection]` object.
    /// * `connection` - existing connection
    /// * `signer` - a [`KeyPair`] of signing account
    pub fn from_connection(connection: Connection, signer: KeyPair) -> Self {
        Self { connection, signer }
    }
    /// Send a transaction to a chain. It waits for a given tx `status`.
    /// * `tx` - encoded transaction payload
    /// * `status` - tx status
    /// # Returns
    /// Block hash of block where transaction was put or error
    /// # Examples
    /// ```rust
    ///      let tx = api::tx()
    ///             .balances()
    ///             .transfer(MultiAddress::Id(dest), amount);
    ///         send_tx(tx, status).await
    /// ```
    pub async fn send_tx<Call: TxPayload>(
        &self,
        tx: Call,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        self.send_tx_with_params(tx, Default::default(), status)
            .await
    }

    /// Send a transaction to a chain. It waits for a given tx `status`.
    /// * `tx` - encoded transaction payload
    /// * `params` - optional tx params e.g. tip
    /// * `status` - tx status
    /// # Returns
    /// Block hash of block where transaction was put or error
    pub async fn send_tx_with_params<Call: TxPayload>(
        &self,
        tx: Call,
        params: BaseExtrinsicParamsBuilder<SubstrateConfig, PlainTip>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        if let Some(details) = tx.validation_details() {
            info!(target:"aleph-client", "Sending extrinsic {}.{} with params: {:?}", details.pallet_name, details.call_name, params);
        }
        let progress = self
            .connection
            .client
            .tx()
            .sign_and_submit_then_watch(&tx, &self.signer, params)
            .await?;

        // In case of Submitted hash does not mean anything
        let hash = match status {
            TxStatus::InBlock => progress.wait_for_in_block().await?.block_hash(),
            TxStatus::Finalized => progress.wait_for_finalized_success().await?.block_hash(),
            TxStatus::Submitted => return Ok(BlockHash::from_low_u64_be(0)),
        };
        info!(target: "aleph-client", "tx included in block {:?}", hash);

        Ok(hash)
    }
}

impl RootConnection {
    /// Creates new root connection from a given url.
    /// By default, it tries to connect 10 times, waiting 1 second between each unsuccessful attempt.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    /// * `root` - a [`KeyPair`] of the Sudo account
    pub async fn new(address: String, root: KeyPair) -> anyhow::Result<Self> {
        RootConnection::try_from_connection(Connection::new(address).await, root).await
    }

    /// Creates new root connection from a given [`Connection`] object. It validates whether given
    /// key is really a sudo account
    /// * `connection` - existing connection
    /// * `signer` - a [`KeyPair`] of the Sudo account
    pub async fn try_from_connection(
        connection: Connection,
        signer: KeyPair,
    ) -> anyhow::Result<Self> {
        let root_address = api::storage().sudo().key();

        let root = match connection.client.storage().fetch(&root_address, None).await {
            Ok(Some(account)) => account,
            _ => return Err(anyhow!("Could not read sudo key from chain")),
        };

        if root != *signer.account_id() {
            return Err(anyhow!(
                "Provided account is not a sudo on chain. sudo key - {}, provided: {}",
                root,
                signer.account_id()
            ));
        }

        Ok(Self {
            connection,
            root: signer,
        })
    }

    /// Converts [`RootConnection`] to [`SignedConnection`]
    pub fn as_signed(&self) -> SignedConnection {
        SignedConnection {
            connection: self.connection.clone(),
            signer: KeyPair::new(self.root.signer().clone()),
        }
    }
}
