use codec::Compact;
use common::create_connection;
use log::info;
use sp_core::{sr25519, Pair};
use sp_runtime::{generic, traits::BlakeTwo256, AccountId32, MultiAddress};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{AccountId, Api, Balance, UncheckedExtrinsicV4};

use crate::config::Config;

pub type BlockNumber = u32;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type KeyPair = sr25519::Pair;
pub type Connection = Api<KeyPair, WsRpcClient>;
pub type TransferTransaction =
    UncheckedExtrinsicV4<([u8; 2], MultiAddress<AccountId, ()>, Compact<u128>)>;

pub fn keypair_from_string(seed: String) -> KeyPair {
    KeyPair::from_string(&seed, None).expect("Can't create pair from seed value")
}

pub fn accounts(seeds: Option<Vec<String>>) -> Vec<KeyPair> {
    let seeds = seeds.unwrap_or_else(|| {
        vec![
            "//Damian".into(),
            "//Tomasz".into(),
            "//Zbyszko".into(),
            "//Hansu".into(),
        ]
    });
    seeds.into_iter().map(keypair_from_string).collect()
}

pub fn get_sudo(config: Config) -> KeyPair {
    match config.sudo {
        Some(seed) => keypair_from_string(seed),
        None => accounts(config.seeds)[0].to_owned(),
    }
}

#[derive(Debug)]
pub struct FeeInfo {
    pub fee_without_weight: Balance,
    pub unadjusted_weight: Balance,
    pub adjusted_weight: Balance,
}

pub fn get_tx_fee_info(connection: &Connection, tx: &TransferTransaction) -> FeeInfo {
    let unadjusted_weight = connection
        .get_payment_info(&tx.hex_encode(), None)
        .unwrap()
        .unwrap()
        .weight as Balance;

    let fee = connection
        .get_fee_details(&tx.hex_encode(), None)
        .unwrap()
        .unwrap();
    let inclusion_fee = fee.inclusion_fee.unwrap();

    FeeInfo {
        fee_without_weight: inclusion_fee.base_fee + inclusion_fee.len_fee + fee.tip,
        unadjusted_weight,
        adjusted_weight: inclusion_fee.adjusted_weight_fee,
    }
}

pub fn get_free_balance(account: &AccountId32, connection: &Connection) -> Balance {
    connection.get_account_data(account).unwrap().unwrap().free
}

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
