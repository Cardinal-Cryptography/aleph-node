use pallet_elections::EraValidators;
use primitives::CommitteeSeats;
use sp_core::H256;
use substrate_api_client::AccountId;

use crate::{AnyConnection, SignedConnection};

const PALLET: &str = "Elections";

pub fn get_committee_seats<C: AnyConnection>(
    connection: &C,
    block_hash: Option<H256>,
) -> CommitteeSeats {
    connection
        .as_connection()
        .get_storage_value(PALLET, "CommitteeSize", block_hash)
        .expect("Failed to obtain CommitteeSize extrinsic!")
        .unwrap_or_else(|| {
            panic!(
                "Failed to decode CommitteeSize for block hash: {:?}.",
                block_hash
            )
        })
}

pub fn get_validator_block_count<C: AnyConnection>(
    connection: &C,
    account_id: &AccountId,
    block_hash: Option<H256>,
) -> Option<u32> {
    connection
        .as_connection()
        .get_storage_map(PALLET, "SessionValidatorBlockCount", account_id, block_hash)
        .expect("Failed to obtain SessionValidatorBlockCount extrinsic!")
}

pub fn get_current_era_validators(connection: &SignedConnection) -> Vec<AccountId> {
    let eras_validators: EraValidators<AccountId> =
        connection.read_storage_value(PALLET, "CurrentEraValidators");
    eras_validators
        .reserved
        .into_iter()
        .chain(eras_validators.non_reserved.into_iter())
        .collect()
}

pub fn get_current_era_reserved_validators(connection: &SignedConnection) -> Vec<AccountId> {
    let eras_validators: EraValidators<AccountId> =
        connection.read_storage_value(PALLET, "CurrentEraValidators");
    eras_validators.reserved
}

pub fn get_current_era_non_reserved_validators(connection: &SignedConnection) -> Vec<AccountId> {
    let eras_validators: EraValidators<AccountId> =
        connection.read_storage_value(PALLET, "CurrentEraValidators");
    eras_validators.non_reserved
}

pub fn get_next_era_reserved_validators(connection: &SignedConnection) -> Vec<AccountId> {
    connection.read_storage_value(PALLET, "NextEraReservedValidators")
}

pub fn get_next_era_non_reserved_validators(connection: &SignedConnection) -> Vec<AccountId> {
    connection.read_storage_value(PALLET, "NextEraNonReservedValidators")
}
