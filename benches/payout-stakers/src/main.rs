use codec::Compact;
use common::{create_connection, Connection};
use e2e::staking::{bond, wait_for_era_completion};
use e2e::{
    send_xt,
    staking::{
        check_non_zero_payouts_for_era, validate, wait_for_full_era_completion, RewardDestination,
    },
    transfer::batch_endow_account_balances,
};
use log::info;
use primitives::TOKEN_DECIMALS;
use rayon::prelude::*;
use sp_core::{sr25519, Pair};
use sp_keyring::AccountKeyring;
use std::iter;
use substrate_api_client::{compose_call, compose_extrinsic, AccountId, GenericAddress, XtStatus};

const TOKEN: u128 = 10u128.pow(TOKEN_DECIMALS);
const VALIDATOR_STAKE: u128 = 25_000 * TOKEN;
const NOMINATOR_STAKE: u128 = 1_000 * TOKEN;
const NOMINATOR_COUNT: u64 = 1024;
const VALIDATOR_COUNT: u64 = 4;

// we need to schedule batches for limited call count, otherwise we'll exhaust a block
const BOND_CALL_BATCH_LIMIT: usize = 256;
const NOMINATE_CALL_BATCH_LIMIT: usize = 128;

// 1. Generate 1024 accounts
// 2. set validators status to Validate
// 3. set them to nominate
// 4. wait a full era and send payout stakers xt; repeat a few times
fn main() -> Result<(), anyhow::Error> {
    let address = "127.0.0.1:9944";
    let sudoer = AccountKeyring::Alice.pair();

    env_logger::init();
    info!("Starting benchmark with config ");

    let connection = create_connection(address).set_signer(sudoer);

    // 1. Generate 1024 accounts
    let accounts = (VALIDATOR_COUNT..NOMINATOR_COUNT + VALIDATOR_COUNT)
        .map(derive_user_account)
        .collect::<Vec<_>>();
    batch_endow_account_balances(&connection, &accounts, TOKEN + NOMINATOR_STAKE);

    // 2. Set validators status to Validate
    let validators = (0..VALIDATOR_COUNT)
        .map(derive_user_account)
        .collect::<Vec<_>>();
    validators.par_iter().for_each(|account| {
        bond(address, VALIDATOR_STAKE, &account, &account);
    });
    validators
        .par_iter()
        .for_each(|account| validate(address, account, XtStatus::InBlock));

    // // 3. Let accounts nominate validator[0]
    let nominee = &validators[0];
    let stash_validators_pairs = accounts.iter().zip(accounts.iter()).collect::<Vec<_>>();
    stash_validators_pairs
        .chunks(BOND_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            batch_bond(
                &connection,
                chunk,
                NOMINATOR_STAKE,
                RewardDestination::Staked,
            )
        });
    let nominator_nominee_pairs = accounts
        .iter()
        .zip(iter::repeat(nominee))
        .collect::<Vec<_>>();
    nominator_nominee_pairs
        .chunks(NOMINATE_CALL_BATCH_LIMIT)
        .for_each(|chunk| batch_nominate(&connection, chunk));

    // 5. wait for era and payout; repeat
    let mut current_era = wait_for_full_era_completion(&connection)?;
    for _ in 0..10 {
        info!(
            "Era {} started, claiming rewards for era {}",
            current_era,
            current_era - 1
        );
        check_non_zero_payouts_for_era(&address.to_owned(), nominee, &connection, current_era);
        current_era = wait_for_era_completion(&connection, current_era + 1)?;
    }

    Ok(())
}

fn derive_user_account(seed: u64) -> sr25519::Pair {
    sr25519::Pair::from_string(&format!("//{}", seed), None).unwrap()
}

pub fn batch_bond(
    connection: &Connection,
    stash_controller_key_pairs: &[(&sr25519::Pair, &sr25519::Pair)],
    bond_value: u128,
    reward_destination: RewardDestination<GenericAddress>,
) {
    let batch_bond_calls: Vec<_> = stash_controller_key_pairs
        .iter()
        .map(|(stash_key, controller_key)| {
            let bond_call = compose_call!(
                connection.metadata,
                "Staking",
                "bond",
                GenericAddress::Id(AccountId::from(controller_key.public())),
                Compact(bond_value),
                reward_destination.clone()
            );
            compose_call!(
                connection.metadata,
                "Sudo",
                "sudo_as",
                GenericAddress::Id(AccountId::from(stash_key.public())),
                bond_call
            )
        })
        .collect::<Vec<_>>();

    let xt = compose_extrinsic!(connection, "Utility", "batch", batch_bond_calls);
    send_xt(
        connection,
        xt.hex_encode(),
        "batch of bond calls",
        XtStatus::InBlock,
    );
}

pub fn batch_nominate(
    connection: &Connection,
    nominator_nominee_pairs: &[(&sr25519::Pair, &sr25519::Pair)],
) {
    let batch_nominate_calls: Vec<_> = nominator_nominee_pairs
        .iter()
        .map(|(nominator, nominee)| {
            let nominate_call = compose_call!(
                connection.metadata,
                "Staking",
                "nominate",
                vec![GenericAddress::Id(AccountId::from(nominee.public()))]
            );
            compose_call!(
                connection.metadata,
                "Sudo",
                "sudo_as",
                GenericAddress::Id(AccountId::from(nominator.public())),
                nominate_call
            )
        })
        .collect::<Vec<_>>();

    let xt = compose_extrinsic!(connection, "Utility", "batch", batch_nominate_calls);
    send_xt(
        connection,
        xt.hex_encode(),
        "batch of nominate calls",
        XtStatus::InBlock,
    );
}
