pub mod fee;
pub mod types;
pub mod accounts;

use common::create_connection;
use log::info;
use sp_core::Pair;
use sp_runtime::AccountId32;
use substrate_api_client::{AccountId, Balance, UncheckedExtrinsicV4};

use crate::config::Config;
use crate::utils::types::{Connection, KeyPair, TransferTransaction};
use crate::utils::accounts::accounts;

pub fn setup_for_transfer(config: Config) -> (Connection, AccountId32, AccountId32) {
    let Config { node, seeds, .. } = config;

    let accounts = accounts(seeds);
    let (from, to) = (accounts[0].to_owned(), accounts[1].to_owned());

    let connection = create_connection(node).set_signer(from.clone());
    let from = AccountId::from(from.public());
    let to = AccountId::from(to.public());
    (connection, from, to)
}

pub fn transfer(target: &AccountId32, value: u128, connection: &Connection) -> TransferTransaction {
    crate::send_extrinsic!(
        connection,
        "Balances",
        "transfer",
        |tx_hash| info!("[+] Transfer transaction hash: {}", tx_hash),
        GenericAddress::Id(target.clone()),
        Compact(value)
    )
}

#[macro_export]
macro_rules! send_extrinsic {
	($connection: expr,
	$module: expr,
	$call: expr,
    $hash_log: expr
	$(, $args: expr) *) => {
		{
            use substrate_api_client::{compose_extrinsic, XtStatus};

            let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
                $connection,
                $module,
                $call
                $(, ($args)) *
            );

            let tx_hash = $connection
                .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
                .unwrap()
                .expect("Could not get tx hash");
            $hash_log(tx_hash);

            tx
		}
    };
}
