use codec::Compact;
use common::create_connection;
use frame_support::PalletId;
use log::info;
use sp_core::{sr25519, Pair};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{generic, traits::BlakeTwo256, AccountId32, MultiAddress};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{
    compose_extrinsic, AccountId, Api, Balance, UncheckedExtrinsicV4, XtStatus,
};

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

pub fn get_first_account(accounts: &[KeyPair]) -> KeyPair {
    accounts.get(0).expect("No accounts passed").to_owned()
}

pub fn get_first_two_accounts(accounts: &[KeyPair]) -> (KeyPair, KeyPair) {
    let first = get_first_account(accounts);
    let second = accounts
        .get(1)
        .expect("Pass at least two accounts")
        .to_owned();
    (first, second)
}

pub fn get_sudo(config: Config) -> KeyPair {
    match config.sudo {
        Some(seed) => keypair_from_string(seed),
        None => get_first_account(&accounts(config.seeds))
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

    let (from, to) = get_first_two_accounts(&accounts(seeds));
    let connection = create_connection(node).set_signer(from.clone());
    let from = AccountId::from(from.public());
    let to = AccountId::from(to.public());
    (connection, from, to)
}

pub fn transfer(target: &AccountId32, value: u128, connection: &Connection) -> TransferTransaction {
    let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
        connection,
        "Balances",
        "transfer",
        GenericAddress::Id(target.clone()),
        Compact(value)
    );

    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
        .unwrap()
        .expect("Could not get tx hash");
    info!("[+] Transfer transaction hash: {}", tx_hash);

    tx
}

pub fn get_total_issuance(connection: &Connection) -> u128 {
    connection
        .get_storage_value("Balances", "TotalIssuance", None)
        .unwrap()
        .unwrap()
}

//////////////////////
// Treasury related //
//////////////////////

pub fn get_treasury_account(connection: &Connection) -> AccountId32 {
    let pallet_id = connection
        .metadata
        .module_with_constants_by_name("Treasury")
        .unwrap()
        .constant_by_name("PalletId")
        .unwrap()
        .get_value();
    PalletId(pallet_id.try_into().unwrap()).into_account()
}

type ProposalTransaction =
    UncheckedExtrinsicV4<([u8; 2], Compact<u128>, MultiAddress<AccountId, ()>)>;
pub fn propose_treasury_spend(
    value: u128,
    beneficiary: &AccountId32,
    connection: &Connection,
) -> ProposalTransaction {
    let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
        connection,
        "Treasury",
        "propose_spend",
        Compact(value),
        GenericAddress::Id(beneficiary.clone())
    );

    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
        .unwrap()
        .expect("Could not get tx hash");
    info!("[+] Treasury spend transaction hash: {}", tx_hash);

    tx
}

pub fn get_proposals_counter(connection: &Connection) -> u32 {
    connection
        .get_storage_value("Treasury", "ProposalCount", None)
        .unwrap()
        .unwrap()
}

type GovernanceTransaction = UncheckedExtrinsicV4<([u8; 2], Compact<u32>)>;
pub fn send_treasury_approval(proposal_id: u32, connection: &Connection) -> GovernanceTransaction {
    let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
        connection,
        "Treasury",
        "approve_proposal",
        Compact(proposal_id)
    );

    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
        .unwrap()
        .expect("Could not get tx hash");
    info!("[+] Treasury approval transaction hash: {}", tx_hash);

    tx
}

pub fn send_treasury_rejection(proposal_id: u32, connection: &Connection) -> GovernanceTransaction {
    let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
        connection,
        "Treasury",
        "reject_proposal",
        Compact(proposal_id)
    );

    let tx_hash = connection
        .send_extrinsic(tx.hex_encode(), XtStatus::Finalized)
        .unwrap()
        .expect("Could not get tx hash");
    info!("[+] Treasury rejection transaction hash: {}", tx_hash);

    tx
}
