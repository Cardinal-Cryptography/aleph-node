use primitives::CommitteeSeats;
use sp_core::H256;
use substrate_api_client::AccountId;

use crate::AnyConnection;

pub fn get_committee_seats<C: AnyConnection>(
    connection: &C,
    block_hash: Option<H256>,
) -> CommitteeSeats {
    connection
        .as_connection()
        .get_storage_value("Elections", "CommitteeSize", block_hash)
        .expect("Failed to decode CommitteeSize extrinsic!")
        .unwrap_or_else(|| {
            panic!(
                "Failed to obtain CommitteeSize for block hash: {:?}.",
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
        .get_storage_map(
            "Elections",
            "SessionValidatorBlockCount",
            account_id,
            block_hash,
        )
        .expect("Failed to decode SessionValidatorBlockCount extrinsic!")
}
