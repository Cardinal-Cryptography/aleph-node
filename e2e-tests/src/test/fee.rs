use aleph_client::{
    keypair_from_string,
    pallets::balances::{BalanceApi, BalanceUserBatchExtApi},
    sp_runtime::FixedU128,
    utility::BlocksApi,
    waiting::{AlephWaiting, BlockStatus},
    AccountId, SignedConnection, TxStatus,
};
use once_cell::sync::Lazy;

use crate::{config::setup_test, fee::current_fees, transfer::setup_for_transfer};

/// In order to increase the block occupancy we need to transfer funds to a lot of accounts. This
/// array contains the accounts we will be transferring funds to.
static DESTINATIONS: Lazy<Vec<AccountId>> = Lazy::new(|| {
    (0..1400)
        .map(|i| keypair_from_string(&format!("//{i}")).account_id().clone())
        .collect::<Vec<_>>()
});

/// The level of occupancy in a block.
enum BlockOccupancy {
    Low,
    High,
}

/// Ensures that the fee multiplier is adjusted according to the block occupancy.
#[tokio::test]
pub async fn fee_calculation() -> anyhow::Result<()> {
    let config = setup_test();
    let (connection, to) = setup_for_transfer(config).await;

    let minimal_multiplier: FixedU128 = FixedU128::from(1);

    let (no_traffic_fee, no_traffic_multiplier) =
        current_fees(&connection, to.clone(), None, 1).await;
    assert_eq!(
        no_traffic_multiplier, minimal_multiplier,
        "In the beginning the fee multiplier should be equal to the minimal value",
    );

    fill_blocks(BlockOccupancy::Low, 10, &connection).await;

    let (low_traffic_fee, low_traffic_multiplier) =
        current_fees(&connection, to.clone(), None, 1).await;
    assert_eq!(
        low_traffic_multiplier, no_traffic_multiplier,
        "Low traffic shouldn't affect the fee multiplier",
    );
    // This might fail if the incremented nonce has longer encoding. Just restart the test.
    assert_eq!(
        no_traffic_fee, low_traffic_fee,
        "Low traffic shouldn't affect the fee"
    );

    fill_blocks(BlockOccupancy::High, 10, &connection).await;

    let (high_traffic_fee, high_traffic_multiplier) =
        current_fees(&connection, to.clone(), None, 1).await;
    assert!(
        high_traffic_multiplier > low_traffic_multiplier,
        "High traffic should lead to higher fee multiplier",
    );
    assert!(
        high_traffic_fee > low_traffic_fee,
        "High traffic should lead to higher fee"
    );

    fill_blocks(BlockOccupancy::High, 10, &connection).await;

    let (highest_traffic_fee, highest_traffic_multiplier) =
        current_fees(&connection, to.clone(), None, 1).await;
    assert!(
        highest_traffic_multiplier > high_traffic_multiplier,
        "High traffic should lead to higher fee multiplier",
    );
    assert!(
        highest_traffic_fee > high_traffic_fee,
        "High traffic should lead to higher fee"
    );

    let now = connection.get_best_block().await.unwrap().unwrap();
    connection
        .wait_for_block(|n| n >= now + 10, BlockStatus::Finalized)
        .await;

    let (after_traffic_fee, after_traffic_multiplier) =
        current_fees(&connection, to.clone(), None, 1).await;
    assert!(
        after_traffic_multiplier < highest_traffic_multiplier,
        "Lower traffic should lead to lower fee multiplier",
    );
    assert!(
        after_traffic_fee < highest_traffic_fee,
        "Lower traffic should lead to lower fee multiplier",
    );

    Ok(())
}

/// Fill blocks with transfers to increase the block occupancy.
///
/// The number of consecutive blocks to fill is specified by `blocks` parameter. The level of
/// occupancy in each block is specified by `block_occupancy` parameter.
///
/// Every batch contains a number of transfers to accounts from `DESTINATIONS` array. The transfer
/// amount is equal to the existential deposit of the chain.
async fn fill_blocks(block_occupancy: BlockOccupancy, blocks: u32, connection: &SignedConnection) {
    let limit = match block_occupancy {
        BlockOccupancy::Low => 140,
        BlockOccupancy::High => 1400,
    };

    let existential_deposit = connection
        .existential_deposit()
        .await
        .expect("Failed to get existential deposit");

    for _ in 0..blocks {
        connection
            .batch_transfer(
                &DESTINATIONS[..limit],
                existential_deposit,
                TxStatus::InBlock,
            )
            .await
            .unwrap_or_else(|err| panic!("Error while submitting batch: {err:?}"));
    }
}