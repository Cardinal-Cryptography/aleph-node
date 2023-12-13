use std::{fs, path::PathBuf};

use aleph_client::{
    pallets::vk_storage::{VerificationKeyIdentifier, VkStorageSudoApi, VkStorageUserApi},
    RootConnection, SignedConnection, TxStatus,
};
use anyhow::Result;

fn read_bytes(file: &PathBuf) -> Result<Vec<u8>> {
    fs::read(file).map_err(|e| e.into())
}

/// Calls `pallet_vk_storage::store_key`.
pub async fn store_key(
    connection: SignedConnection,
    identifier: VerificationKeyIdentifier,
    vk_file: PathBuf,
) -> Result<()> {
    let vk = read_bytes(&vk_file)?;
    connection
        .store_key(identifier, vk, TxStatus::Finalized)
        .await
        .map(|_| ())
}

/// Calls `pallet_vk_storage::delete_key`.
pub async fn delete_key(
    connection: RootConnection,
    identifier: VerificationKeyIdentifier,
) -> Result<()> {
    connection
        .delete_key(identifier, TxStatus::Finalized)
        .await
        .map(|_| ())
}

/// Calls `pallet_vk_storage::overwrite_key`.
pub async fn overwrite_key(
    connection: RootConnection,
    identifier: VerificationKeyIdentifier,
    vk_file: PathBuf,
) -> Result<()> {
    let vk = read_bytes(&vk_file)?;
    connection
        .overwrite_key(identifier, vk, TxStatus::Finalized)
        .await
        .map(|_| ())
}
