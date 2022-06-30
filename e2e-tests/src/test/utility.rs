use crate::{config::Config, transfer::setup_for_transfer};
use aleph_client::{get_exposure, AnyConnection, SignedConnection};
use codec::Compact;
use log::info;
use pallet_staking::Exposure;
use primitives::EraIndex;
use sp_core::{Pair, H256};
use substrate_api_client::{
    compose_call, compose_extrinsic, AccountId, ExtrinsicParams, GenericAddress, XtStatus,
};

pub fn batch_transactions(config: &Config) -> anyhow::Result<()> {
    const NUMBER_OF_TRANSACTIONS: usize = 100;

    let (connection, to) = setup_for_transfer(config);

    let call = compose_call!(
        connection.as_connection().metadata,
        "Balances",
        "transfer",
        GenericAddress::Id(to),
        Compact(1000u128)
    );
    let mut transactions = Vec::new();
    for _i in 0..NUMBER_OF_TRANSACTIONS {
        transactions.push(call.clone());
    }

    let extrinsic =
        compose_extrinsic!(connection.as_connection(), "Utility", "batch", transactions);

    let finalized_block_hash = connection
        .as_connection()
        .send_extrinsic(extrinsic.hex_encode(), XtStatus::Finalized)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");
    info!(
        "[+] A batch of {} transactions was included in finalized {} block.",
        NUMBER_OF_TRANSACTIONS, finalized_block_hash
    );

    Ok(())
}

pub fn download_exposure(
    connection: &SignedConnection,
    era: EraIndex,
    account_id: &AccountId,
    beginning_of_session_block_hash: H256,
) -> u128 {
    let exposure: Exposure<AccountId, u128> = get_exposure(
        connection,
        era,
        account_id,
        Some(beginning_of_session_block_hash),
    );
    info!(
        "Validator {} has own exposure of {} and total of {}.",
        account_id, exposure.own, exposure.total
    );
    exposure.others.iter().for_each(|individual_exposure| {
        info!(
            "Validator {} has nominator {} exposure {}.",
            account_id, individual_exposure.who, individual_exposure.value
        )
    });
    exposure.total
}
