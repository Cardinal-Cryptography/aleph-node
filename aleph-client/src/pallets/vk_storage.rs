use anyhow::Result;

/// Verification key identifier alias, copied from `pallet_vk_storage`.
pub type VerificationKeyIdentifier = [u8; 8];

use crate::{
    aleph_runtime::RuntimeCall::VkStorage,
    api,
    pallet_vk_storage::pallet::Call::{delete_key, overwrite_key},
    RootConnection, SignedConnection, SignedConnectionApi, SudoCall, TxInfo, TxStatus,
};

/// Pallet vk storage API.
#[async_trait::async_trait]
pub trait VkStorageUserApi {
    /// Store verifying key in pallet's storage.
    async fn store_key(
        &self,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
        status: TxStatus,
    ) -> Result<TxInfo>;
}

/// Pallet vk storage API that requires sudo.
#[async_trait::async_trait]
pub trait VkStorageSudoApi {
    /// Delete verifying key from pallet's storage.
    async fn delete_key(
        &self,
        identifier: VerificationKeyIdentifier,
        status: TxStatus,
    ) -> Result<TxInfo>;

    /// Overwrite verifying key in pallet's storage.
    async fn overwrite_key(
        &self,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
        status: TxStatus,
    ) -> Result<TxInfo>;
}

#[async_trait::async_trait]
impl VkStorageUserApi for SignedConnection {
    async fn store_key(
        &self,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
        status: TxStatus,
    ) -> Result<TxInfo> {
        let tx = api::tx().vk_storage().store_key(identifier, key);
        self.send_tx(tx, status).await
    }
}

#[async_trait::async_trait]
impl VkStorageSudoApi for RootConnection {
    async fn delete_key(
        &self,
        identifier: VerificationKeyIdentifier,
        status: TxStatus,
    ) -> Result<TxInfo> {
        let call = VkStorage(delete_key { identifier });
        self.sudo_unchecked(call, status).await
    }

    async fn overwrite_key(
        &self,
        identifier: VerificationKeyIdentifier,
        key: Vec<u8>,
        status: TxStatus,
    ) -> Result<TxInfo> {
        let call = VkStorage(overwrite_key { identifier, key });
        self.sudo_unchecked(call, status).await
    }
}
