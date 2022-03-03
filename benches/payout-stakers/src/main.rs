use common::create_connection;
use e2e::{
    staking::{
        check_non_zero_payouts_for_era, nominate, validate, wait_for_full_era_completion,
        RewardDestination,
    },
    transfer::batch_endow_account_balances,
};
use log::info;
use primitives::TOKEN_DECIMALS;
use rayon::prelude::*;
use sp_core::{sr25519, Pair};
use sp_keyring::AccountKeyring;
use substrate_api_client::{AccountId, GenericAddress, XtStatus};

const TOKEN: u128 = 10u128.pow(TOKEN_DECIMALS);
const NOMINATOR_STAKE: u128 = 1_000 * TOKEN;

// Plan:
//
// 1. Generate 1024 accounts
// 2. set validators status to Validate
// 3. set them to nominate
// 4. wait a full era
// 5. send payout stakers xt
// 6. TODO repeat 4,5 N times

fn main() -> Result<(), anyhow::Error> {
    let address = "127.0.0.1:9944";
    let sudoer = AccountKeyring::Alice.pair();

    env_logger::init();
    info!("Starting benchmark with config ");

    let connection = create_connection(address).set_signer(sudoer);

    // 1. Generate 1024 accounts
    let accounts = (0..1024).map(derive_user_account).collect::<Vec<_>>();
    batch_endow_account_balances(&connection, &accounts, TOKEN);

    // 2. Set validators status to Validate
    let validators = (0..4).map(derive_user_account).collect::<Vec<_>>();
    validators
        .par_iter()
        .for_each(|account| validate(address, account, XtStatus::InBlock));

    // 3. Let accounts nominate validator[0]
    let nominee = &validators[0];
    accounts
        .par_iter()
        .for_each(|nominator| bond(address, NOMINATOR_STAKE, nominator));

    accounts
        .par_iter()
        .for_each(|nominator| nominate(address, nominator, nominee));

    // 4. Wait full era
    let current_era = wait_for_full_era_completion(&connection)?;
    info!(
        "Era {} started, claiming rewards for era {}",
        current_era,
        current_era - 1
    );

    // 5. payout
    check_non_zero_payouts_for_era(&address.to_owned(), nominee, &connection, current_era);

    // TODO repeat 4,5 N times
    Ok(())
}

pub fn bond(address: &str, stake: u128, account: &sr25519::Pair) {
    let account_id = GenericAddress::Id(AccountId::from(account.public()));
    let connection = create_connection(address).set_signer(account.clone());
    let xt = connection.staking_bond(account_id, stake, RewardDestination::Staked);

    connection.send_extrinsic(xt.hex_encode(), XtStatus::SubmitOnly);
}
fn derive_user_account(seed: u64) -> sr25519::Pair {
    sr25519::Pair::from_string(&format!("//{}", seed), None).unwrap()
}
// // 3. Let accounts nominate validator[0]
// let nominee = &validators[0];
// let nominee_id = GenericAddress::Id(AccountId::from(nominee.public()));
// let calls = accounts
//     .iter()
//     .map(|nominator| {
//         let nominator_id = GenericAddress::Id(AccountId::from(nominator.public()));
//         let call = compose_call!(
//             connection.metadata,
//             "Staking",
//             "bond",
//             nominee_id.clone(),
//             Compact(NOMINATOR_STAKE),
//             RewardDestination::<GenericAddress>::Staked
//         );
//         compose_extrinsic!(connection, "Sudo", "sudo_as", nominator_id, call);
//     })
//     .collect::<Vec<_>>();
// let xt = compose_extrinsic!(connection, "Utility", "batch", calls);
// // let call = compose_call!(connection.metadata, "Utility", "batch", calls);
// // let xt = compose_extrinsic!(connection, "Sudo", "sudo_unchecked_weight", call, 0_u64);
// send_xt(
//     &connection,
//     xt.hex_encode(),
//     "batch of endow balances",
//     XtStatus::InBlock,
// );

// accounts
//     .par_iter()
//     .for_each(|nominator| nominate(address, nominator, nominee));
