use log::info;

use substrate_api_client::{compose_call, compose_extrinsic, GenericAddress, XtStatus};
use sp_core::Pair;
use crate::transfer::{setup_for_transfer};
use std::fmt::Write;

use codec::{Compact};

use crate::config::Config;

pub fn batch_transactions(config: Config) -> anyhow::Result<()> {
    const NUMBER_OF_TRANSACTIONS : usize = 100;

    let (connection, _, to) = setup_for_transfer(config);

    let call = compose_call!(
        connection.metadata,
        "Balances",
        "transfer",
        GenericAddress::Id(to.clone()),
        Compact(1000u128)
    );
    let mut transactions = Vec::new();
    for _i in 0..NUMBER_OF_TRANSACTIONS {
        transactions.push(call.clone());
    }

    let extrinsic = compose_extrinsic!(
        connection,
        "Utility",
        "batch",
        transactions
    );

    let finalized_block_hash = connection
        .send_extrinsic(extrinsic.hex_encode(), XtStatus::Finalized)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");
    info!("[+] A batch of {} transactions was included in {} block.", NUMBER_OF_TRANSACTIONS, finalized_block_hash);

    let block_with_batch_tx = connection.get_block::<aleph_runtime::Block>(Some(finalized_block_hash)).
        expect("Could not get extrinsic").
        expect("Coult not get block");

    // a crude way of doing things - how to access members of Call directly?
    let unchecked_batch_tx_extrinsic = &block_with_batch_tx.extrinsics;
    let mut extrinsics_string = String::new();
    write!(&mut extrinsics_string, "[+] {:?}", unchecked_batch_tx_extrinsic)?;
    assert_eq!(extrinsics_string.matches("Call::Balances(Call::transfer(").count(), NUMBER_OF_TRANSACTIONS);

    Ok(())
}
