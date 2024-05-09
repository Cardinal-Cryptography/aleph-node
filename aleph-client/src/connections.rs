use std::{thread::sleep, time::Duration};

use anyhow::anyhow;
use codec::Decode;
use log::{debug, info};
use primitives::Nonce;
use serde::{Deserialize, Serialize};
use subxt::{
    blocks::ExtrinsicEvents,
    ext::sp_core::Bytes,
    metadata::DecodeWithMetadata,
    rpc::RpcParams,
    storage::{
        address::{Address, StaticStorageMapKey, Yes},
        StorageAddress,
    },
    tx::{SubmittableExtrinsic as SubxtSubmittable, TxPayload},
};

use crate::{
    api, runtime_types::sp_weights::weight_v2::Weight, AccountId, AlephConfig, BlockHash, Call,
    KeyPair, ParamsBuilder, SubxtClient, TxHash, TxStatus,
};

/// Capable of communicating with a live Aleph chain.
#[derive(Clone)]
pub struct Connection {
    /// inner subxt type
    pub client: SubxtClient,
}

/// Any connection that is signed by some key.
#[derive(Clone)]
pub struct SignedConnection {
    /// vanilla connection
    pub connection: Connection,
    /// signing authority
    pub signer: KeyPair,
}

/// Specific connection that is signed by the sudo key.
#[derive(Clone)]
pub struct RootConnection {
    connection: SignedConnection,
}

/// Castability to a plain connection.
pub trait AsConnection {
    /// Allows cast to [`Connection`] reference
    fn as_connection(&self) -> &Connection;
}

/// Castability to a signed connection.
pub trait AsSigned {
    /// Allows cast to [`SignedConnection`] reference
    fn as_signed(&self) -> &SignedConnection;
}

/// Any connection should be able to request storage and submit RPC calls
#[async_trait::async_trait]
pub trait ConnectionApi: Sync {
    /// Retrieves a decoded storage value stored under given key.
    ///
    /// # Panic
    /// This method `panic`s, in case storage key is invalid, or in case value cannot be decoded,
    /// or there is no such value
    /// * `addrs` - represents a storage key, see [more info about keys](https://docs.substrate.io/fundamentals/state-transitions-and-storage/#querying-storage)
    /// * `at` - optional block hash to query state from
    async fn get_storage_entry<T: DecodeWithMetadata + Sync, Defaultable: Sync, Iterable: Sync>(
        &self,
        addrs: &Address<StaticStorageMapKey, T, Yes, Defaultable, Iterable>,
        at: Option<BlockHash>,
    ) -> T;

    /// Retrieves a decoded storage value stored under given key.
    ///
    /// # Panic
    /// This method `panic`s, in case storage key is invalid, or in case value cannot be decoded,
    /// but does _not_ `panic` if there is no such value
    /// * `addrs` - represents a storage key, see [more info about keys](https://docs.substrate.io/fundamentals/state-transitions-and-storage/#querying-storage)
    /// * `at` - optional block hash to query state from
    ///
    /// # Examples
    /// ```ignore
    ///     let addrs = api::storage().treasury().proposal_count();
    ///     get_storage_entry_maybe(&addrs, None).await
    /// ```
    async fn get_storage_entry_maybe<
        T: DecodeWithMetadata + Sync,
        Defaultable: Sync,
        Iterable: Sync,
    >(
        &self,
        addrs: &Address<StaticStorageMapKey, T, Yes, Defaultable, Iterable>,
        at: Option<BlockHash>,
    ) -> Option<T>;

    /// Submit a RPC call.
    ///
    /// * `func_name` - name of a RPC call
    /// * `params` - result of calling `rpc_params!` macro, that's `Vec<u8>` of encoded data
    /// to this rpc call
    ///
    /// # Examples
    /// ```ignore
    ///  let args = ContractCallArgs {
    ///             origin: address.clone(),
    ///             dest: address.clone(),
    ///             value: 0,
    ///             gas_limit: None,
    ///             input_data: payload,
    ///             storage_deposit_limit: None,
    ///         };
    /// let params = rpc_params!["ContractsApi_call", Bytes(args.encode())];
    /// rpc_call("state_call".to_string(), params).await;
    /// ```
    async fn rpc_call<R: Decode>(&self, func_name: String, params: RpcParams) -> anyhow::Result<R>;

    /// Same as [rpc_call] but used for rpc endpoint that does not return values.
    async fn rpc_call_no_return(&self, func_name: String, params: RpcParams) -> anyhow::Result<()>;
}

/// Data regarding submitted transaction.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct TxInfo {
    /// Hash of the block containing tx.
    pub block_hash: BlockHash,
    /// Hash of the transaction itself.
    pub tx_hash: TxHash,
}

impl From<ExtrinsicEvents<AlephConfig>> for TxInfo {
    fn from(ee: ExtrinsicEvents<AlephConfig>) -> Self {
        Self {
            block_hash: ee.block_hash(),
            tx_hash: ee.extrinsic_hash(),
        }
    }
}

/// A signed extrinsics ready to be submitted.
pub struct SubmittableExtrinsic {
    submittable: SubxtSubmittable<AlephConfig, SubxtClient>,
}

impl From<SubxtSubmittable<AlephConfig, SubxtClient>> for SubmittableExtrinsic {
    fn from(submittable: SubxtSubmittable<AlephConfig, SubxtClient>) -> Self {
        Self { submittable }
    }
}

/// Signed connection should be able to sends transactions to chain
#[async_trait::async_trait]
pub trait SignedConnectionApi: ConnectionApi {
    /// Send a transaction to a chain. It waits for a given tx `status`.
    /// * `tx` - encoded transaction payload
    /// * `status` - a [`TxStatus`] for a tx to wait for
    /// # Returns
    /// Block hash of block where transaction was put together with transaction hash, or error.
    /// # Examples
    /// ```ignore
    ///     let tx = api::tx()
    ///         .balances()
    ///         .transfer(MultiAddress::Id(dest), amount);
    ///     send_tx(tx, status).await
    /// ```
    async fn send_tx<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo>;

    /// Send a transaction to a chain. It waits for a given tx `status`.
    /// * `tx` - encoded transaction payload
    /// * `params` - optional tx params e.g. tip
    /// * `status` - a [`TxStatus`] of a tx to wait for
    /// # Returns
    /// Block hash of block where transaction was put together with transaction hash, or error.
    async fn send_tx_with_params<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        params: ParamsBuilder,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo>;

    /// Returns account id which signs this connection
    fn account_id(&self) -> &AccountId;

    /// Returns a [`KeyPair`] which signs this connection
    fn signer(&self) -> &KeyPair;

    /// Tries to convert [`SignedConnection`] as [`RootConnection`]
    async fn try_as_root(&self) -> anyhow::Result<RootConnection> {
        Err(anyhow!("This connenction is not upgradeable to root"))
    }
}

/// Extensions to Signed Connections
pub trait SignedConnectionApiExt: SignedConnectionApi {
    /// Lower level api: signs a transaction with given params and nonce.
    /// * `tx` - encoded transaction payload
    /// * `params` - optional tx params e.g. tip
    /// * `nonce` - tx nonce.
    /// # Returns
    /// A signed transaction ready to be submitted via this connection.
    fn sign_with_params<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        params: ParamsBuilder,
        nonce: Nonce,
    ) -> anyhow::Result<SubmittableExtrinsic>;
}

/// API for [sudo pallet](https://paritytech.github.io/substrate/master/pallet_sudo/index.html).
#[async_trait::async_trait]
pub trait SudoCall {
    /// API for [`sudo_unchecked_weight`](https://paritytech.github.io/substrate/master/pallet_sudo/pallet/enum.Call.html#variant.sudo_unchecked_weight) call.
    async fn sudo_unchecked(&self, call: Call, status: TxStatus) -> anyhow::Result<TxInfo>;
    /// API for [`sudo`](https://paritytech.github.io/substrate/master/pallet_sudo/pallet/enum.Call.html#variant.sudo) call.
    async fn sudo(&self, call: Call, status: TxStatus) -> anyhow::Result<TxInfo>;
}

#[async_trait::async_trait]
impl SudoCall for RootConnection {
    async fn sudo_unchecked(&self, call: Call, status: TxStatus) -> anyhow::Result<TxInfo> {
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

    async fn sudo(&self, call: Call, status: TxStatus) -> anyhow::Result<TxInfo> {
        info!(target: "aleph-client", "sending call as sudo {:?}", call);
        let sudo = api::tx().sudo().sudo(call);

        self.as_signed().send_tx(sudo, status).await
    }
}

impl AsConnection for Connection {
    fn as_connection(&self) -> &Connection {
        self
    }
}

impl<S: AsSigned> AsConnection for S {
    fn as_connection(&self) -> &Connection {
        &self.as_signed().connection
    }
}

impl AsSigned for SignedConnection {
    fn as_signed(&self) -> &SignedConnection {
        self
    }
}

impl AsSigned for RootConnection {
    fn as_signed(&self) -> &SignedConnection {
        &self.connection
    }
}

#[async_trait::async_trait]
impl<C: AsConnection + Sync> ConnectionApi for C {
    async fn get_storage_entry<T: DecodeWithMetadata + Sync, Defaultable: Sync, Iterable: Sync>(
        &self,
        addrs: &Address<StaticStorageMapKey, T, Yes, Defaultable, Iterable>,
        at: Option<BlockHash>,
    ) -> T {
        self.get_storage_entry_maybe(addrs, at)
            .await
            .expect("There should be a value")
    }

    async fn get_storage_entry_maybe<
        T: DecodeWithMetadata + Sync,
        Defaultable: Sync,
        Iterable: Sync,
    >(
        &self,
        addrs: &Address<StaticStorageMapKey, T, Yes, Defaultable, Iterable>,
        at: Option<BlockHash>,
    ) -> Option<T> {
        info!(target: "aleph-client", "accessing storage at {}::{} at block {:?}", addrs.pallet_name(), addrs.entry_name(), at);

        let storage = self.as_connection().as_client().storage();
        let block = match at {
            Some(block_hash) => storage.at(block_hash),
            None => storage.at_latest().await.expect("Should access storage"),
        };

        block.fetch(addrs).await.expect("Should access storage")
    }

    async fn rpc_call<R: Decode>(&self, func_name: String, params: RpcParams) -> anyhow::Result<R> {
        info!(target: "aleph-client", "submitting rpc call `{}`, with params {:?}", func_name, params.clone().build());
        let bytes: Bytes = self
            .as_connection()
            .as_client()
            .rpc()
            .request(&func_name, params)
            .await?;

        Ok(R::decode(&mut bytes.as_ref())?)
    }

    async fn rpc_call_no_return(&self, func_name: String, params: RpcParams) -> anyhow::Result<()> {
        info!(target: "aleph-client", "submitting rpc call `{}`, with params {:?}", func_name, params.clone().build());
        let _: () = self
            .as_connection()
            .as_client()
            .rpc()
            .request(&func_name, params)
            .await?;

        Ok(())
    }
}

impl SubmittableExtrinsic {
    /// Submits a given extrinsic to the chain.
    /// * `status` - a [`TxStatus`] of a tx to wait for.
    ///   In case of TxStatus::Submitted result.block_hash does not mean anything.
    pub async fn submit(&self, status: TxStatus) -> anyhow::Result<TxInfo> {
        Ok(match status {
            TxStatus::InBlock => self
                .submittable
                .submit_and_watch()
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?
                .into(),
            TxStatus::Finalized => self
                .submittable
                .submit_and_watch()
                .await?
                .wait_for_finalized_success()
                .await?
                .into(),

            TxStatus::Submitted => {
                let tx_hash = self.submittable.submit().await?;
                TxInfo {
                    block_hash: Default::default(),
                    tx_hash,
                }
            }
        })
    }
}

#[async_trait::async_trait]
impl<S: AsSigned + Sync> SignedConnectionApi for S {
    async fn send_tx<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo> {
        self.send_tx_with_params(tx, Default::default(), status)
            .await
    }

    async fn send_tx_with_params<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        params: ParamsBuilder,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo> {
        if let Some(details) = tx.validation_details() {
            info!(
                target:"aleph-client", "Sending extrinsic {}.{} with params: {:?}",
                details.pallet_name,
                details.call_name,
                params,
            );
        }

        let signed: SubmittableExtrinsic = self
            .as_connection()
            .as_client()
            .tx()
            .create_signed(&tx, &self.as_signed().signer().inner, params)
            .await?
            .into();
        let info = signed.submit(status).await?;
        info!(target: "aleph-client", "tx with hash {:?} included in block {:?}", info.tx_hash, info.block_hash);

        Ok(info)
    }

    fn account_id(&self) -> &AccountId {
        self.as_signed().signer().account_id()
    }

    fn signer(&self) -> &KeyPair {
        &self.as_signed().signer
    }

    async fn try_as_root(&self) -> anyhow::Result<RootConnection> {
        let temp = self.as_signed().clone();
        RootConnection::try_from_connection(temp.connection, temp.signer).await
    }
}

impl<S: AsSigned + Sync> SignedConnectionApiExt for S {
    fn sign_with_params<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        params: ParamsBuilder,
        nonce: Nonce,
    ) -> anyhow::Result<SubmittableExtrinsic> {
        Ok(self
            .as_connection()
            .as_client()
            .tx()
            .create_signed_with_nonce(&tx, &self.as_signed().signer().inner, nonce.into(), params)?
            .into())
    }
}

impl Connection {
    const DEFAULT_RETRIES: u32 = 10;
    const RETRY_WAIT_SECS: u64 = 6;

    /// Creates new connection from a given url.
    /// By default, it tries to connect 10 times, waiting 1 second between each unsuccessful attempt.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    pub async fn new(address: &str) -> Connection {
        Self::new_with_retries(address, Self::DEFAULT_RETRIES).await
    }

    /// Creates new connection from a given url and given number of connection attempts.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    /// * `retries` - number of connection attempts
    async fn new_with_retries(address: &str, mut retries: u32) -> Connection {
        loop {
            debug!(target: "aleph-client", "new_with_retries: address={address} retries_left={retries}");
            let client = SubxtClient::from_url(&address).await;
            match (retries, client) {
                (_, Ok(client)) => return Connection { client },
                (0, Err(e)) => panic!("new_with_retries failed for address {address}: {e:?}"),
                _ => {
                    sleep(Duration::from_secs(Self::RETRY_WAIT_SECS));
                    retries -= 1;
                }
            }
        }
    }

    /// Casts self to the underlying RPC client.
    pub fn as_client(&self) -> &SubxtClient {
        &self.client
    }
}

impl SignedConnection {
    /// Creates new signed connection from existing [`Connection`] object.
    /// * `connection` - existing connection
    /// * `signer` - a [`KeyPair`] of signing account
    pub async fn new(address: &str, signer: KeyPair) -> Self {
        Self::from_connection(Connection::new(address).await, signer)
    }

    /// Creates new signed connection from existing [`Connection`] object.
    /// * `connection` - existing connection
    /// * `signer` - a [`KeyPair`] of signing account
    pub fn from_connection(connection: Connection, signer: KeyPair) -> Self {
        Self { connection, signer }
    }
}

impl RootConnection {
    /// Creates new root connection from a given url.
    /// It tries to connect 10 times, waiting 1 second between each unsuccessful attempt.
    /// * `address` - address in websocket format, e.g. `ws://127.0.0.1:9943`
    /// * `root` - a [`KeyPair`] of the Sudo account
    pub async fn new(address: &str, root: KeyPair) -> anyhow::Result<Self> {
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

        let root = match connection
            .as_client()
            .storage()
            .at_latest()
            .await?
            .fetch(&root_address)
            .await
        {
            Ok(Some(account)) => account,
            _ => return Err(anyhow!("Could not read sudo key from chain")),
        }
        .0;

        if root != *signer.account_id() {
            return Err(anyhow!(
                "Provided account is not a sudo on chain. sudo key - {}, provided: {}",
                root,
                signer.account_id()
            ));
        }

        Ok(Self {
            connection: SignedConnection { connection, signer },
        })
    }
}
