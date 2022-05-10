use anyhow::Result as AnyResult;
use log::info;
pub use pallet_vesting::VestingInfo;
use sp_core::Pair;
use substrate_api_client::{compose_extrinsic, GenericAddress, XtStatus::Finalized};
use thiserror::Error;

use primitives::Balance;

use crate::{account_from_keypair, try_send_xt, AccountId, BlockNumber, Connection, KeyPair};

/// Gathers errors from this module.
#[derive(Debug, Error)]
pub enum VestingError {
    #[error("🦺❌ Account has no active vesting schedules.")]
    NotVesting,
}
const PALLET: &str = "Vesting";
pub type VestingSchedule = VestingInfo<Balance, BlockNumber>;

pub fn vest(connection: &Connection, who: KeyPair) -> AnyResult<()> {
    // Ensure that we make a call as `account`.
    let connection = connection.clone().set_signer(who.clone());
    let xt = compose_extrinsic!(connection, "Vesting", "vest");
    let block_hash = try_send_xt(&connection, xt, Some("Vesting"), Finalized)?
        .expect("For `Finalized` status a block hash should be returned");
    info!(
        target: "aleph-client", "Vesting for the account {:?}. Finalized in block {:?}",
        account_from_keypair(&who), block_hash
    );
    Ok(())
}

pub fn vest_other(
    connection: &Connection,
    caller: KeyPair,
    vest_account: AccountId,
) -> AnyResult<()> {
    // Ensure that we make a call as `caller`.
    let connection = connection.clone().set_signer(caller);
    let xt = compose_extrinsic!(
        connection,
        PALLET,
        "vest_other",
        GenericAddress::Id(vest_account.clone())
    );
    let block_hash = try_send_xt(&connection, xt, Some("Vesting on behalf"), Finalized)?
        .expect("For `Finalized` status a block hash should be returned");
    info!(target: "aleph-client", "Vesting on behalf of the account {:?}. Finalized in block {:?}", vest_account, block_hash);
    Ok(())
}

pub fn vested_transfer(
    connection: &Connection,
    sender: KeyPair,
    receiver: AccountId,
    schedule: VestingSchedule,
) -> AnyResult<()> {
    // Ensure that we make a call as `sender`.
    let connection = connection.clone().set_signer(sender);
    let xt = compose_extrinsic!(
        connection,
        PALLET,
        "vested_transfer",
        GenericAddress::Id(receiver.clone()),
        schedule
    );
    let block_hash = try_send_xt(&connection, xt, Some("Vested transfer"), Finalized)?
        .expect("For `Finalized` status a block hash should be returned");
    info!(target: "aleph-client", "Vested transfer to the account {:?}. Finalized in block {:?}", receiver, block_hash);
    Ok(())
}

pub fn get_schedules(connection: &Connection, who: AccountId) -> AnyResult<Vec<VestingSchedule>> {
    connection
        .get_storage_map::<AccountId, Vec<VestingSchedule>>(PALLET, "Vesting", who, None)?
        .ok_or_else(|| VestingError::NotVesting.into())
}

pub fn merge_schedules(
    connection: &Connection,
    who: KeyPair,
    idx1: u32,
    idx2: u32,
) -> AnyResult<()> {
    // Ensure that we make a call as `who`.
    let connection = connection.clone().set_signer(who.clone());
    let xt = compose_extrinsic!(connection, PALLET, "merge_schedules", idx1, idx2);
    let block_hash = try_send_xt(&connection, xt, Some("Merge vesting schedules"), Finalized)?
        .expect("For `Finalized` status a block hash should be returned");
    info!(target: "aleph-client", 
        "Merging vesting schedules (indices: {} and {}) for the account {:?}. Finalized in block {:?}", 
        idx1, idx2, account_from_keypair(&who), block_hash);
    Ok(())
}
