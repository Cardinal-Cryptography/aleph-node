use std::{thread::sleep, time::Duration};

use aleph_client::{
    pallets::{elections::ElectionsSudoApi, session::SessionApi},
    primitives::CommitteeSeats,
    utility::BlocksApi,
    waiting::{BlockStatus, WaitingExt},
    AccountId, AsConnection, Pair, SignedConnection, TxStatus,
};
use log::info;

use crate::{
    accounts::{get_validator_seed, get_validators_keys, NodeKeys},
    config::setup_test,
    rewards::{reset_validator_keys, set_invalid_keys_for_validator},
};

/// time needed for 4 out of 5 block producers to do 3 sessions.
const SLEEP_DURATION: Duration = Duration::from_secs(108);

async fn prepare_test() -> anyhow::Result<()> {
    let config = setup_test();

    let accounts = get_validators_keys(config);
    let connection = config.create_root_connection().await;

    let new_validators: Vec<AccountId> = accounts
        .iter()
        .map(|pair| pair.signer().public().into())
        .collect();

    let seats = CommitteeSeats {
        reserved_seats: 3,
        non_reserved_seats: 2,
        non_reserved_finality_seats: 1,
    };

    connection
        .change_validators(
            Some(new_validators[0..3].to_vec()),
            Some(new_validators[3..].to_vec()),
            Some(seats),
            TxStatus::InBlock,
        )
        .await?;

    Ok(())
}

fn validator_address(index: u32) -> String {
    const BASE: &str = "ws://127.0.0.1";
    const FIRST_PORT: u32 = 9944;

    let port = FIRST_PORT + index;

    format!("{BASE}:{port}")
}

async fn disable_validator(index: u32) -> anyhow::Result<()> {
    info!("Disabling #{index} validator");
    let validator_seed = get_validator_seed(index);
    let stash_controller = NodeKeys::from(validator_seed);
    let controller_key_to_disable = stash_controller.controller;
    let address = validator_address(index);

    // This connection has to be set up with the controller key.
    let connection_to_disable = SignedConnection::new(&address, controller_key_to_disable).await;

    set_invalid_keys_for_validator(&connection_to_disable).await
}

async fn resurect_validator(index: u32) -> anyhow::Result<()> {
    info!("Resurecting #{index} validator");
    let validator_seed = get_validator_seed(index);
    let stash_controller = NodeKeys::from(validator_seed);
    let controller_key = stash_controller.controller;
    let address = validator_address(index);

    // This connection has to be set up with the controller key.
    let controller_connection = SignedConnection::new(&address, controller_key).await;

    reset_validator_keys(&controller_connection).await
}

#[tokio::test]
async fn split_test() -> anyhow::Result<()> {
    let config = setup_test();
    let root_connection = config.create_root_connection().await;
    let connection = root_connection.as_connection();
    prepare_test().await?;

    connection.wait_for_n_eras(2, BlockStatus::Finalized).await;

    // For each reserved node disable it and check that block finalization stopped.
    // To check that we check that at most 2 sessions passed after disabling - we have limit of 20 blocks
    // created after last finalized block.
    for i in 0..3 {
        info!("Testing if #{i} reserved validator is in finalization committee");
        disable_validator(i).await?;
        let session_before = connection.get_session(None).await;
        let block_before = connection
            .get_best_block()
            .await?
            .expect("there should be some block");
        sleep(SLEEP_DURATION);
        let session_after = connection.get_session(None).await;
        let block_after = connection
            .get_best_block()
            .await?
            .expect("there should be some block");
        assert!(session_before + 2 >= session_after);
        // at least some blocks were produced after disabling
        assert!(block_after > block_before + 10);
        resurect_validator(i).await?;
        connection
            .wait_for_n_sessions(1, BlockStatus::Finalized)
            .await;
    }

    Ok(())
}
