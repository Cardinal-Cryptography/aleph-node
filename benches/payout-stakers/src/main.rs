use common::create_connection;
use e2e::{
    accounts::derive_user_account,
    staking::{
        batch_bond, batch_nominate, bond, check_non_zero_payouts_for_era, validate,
        wait_for_era_completion, wait_for_full_era_completion, RewardDestination,
    },
    transfer::batch_endow_account_balances,
};
use log::info;
use primitives::{
    staking::{MIN_NOMINATOR_BOND, MIN_VALIDATOR_BOND},
    TOKEN,
};
use rayon::prelude::*;
use sp_keyring::AccountKeyring;
use std::iter;
use substrate_api_client::XtStatus;

const NOMINATOR_COUNT: u64 = 1024;
const VALIDATOR_COUNT: u64 = 4;
// we need to schedule batches for limited call count, otherwise we'll exhaust a block max weight
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
    batch_endow_account_balances(&connection, &accounts, TOKEN + MIN_NOMINATOR_BOND);

    // 2. Set validators status to Validate
    let validators = (0..VALIDATOR_COUNT)
        .map(derive_user_account)
        .collect::<Vec<_>>();
    validators.par_iter().for_each(|account| {
        bond(address, MIN_VALIDATOR_BOND, &account, &account);
    });
    validators
        .par_iter()
        .for_each(|account| validate(address, account, XtStatus::InBlock));

    // 3. Let accounts nominate validator[0]
    let nominee = &validators[0];
    let stash_validators_pairs = accounts.iter().zip(accounts.iter()).collect::<Vec<_>>();
    stash_validators_pairs
        .chunks(BOND_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            batch_bond(
                &connection,
                chunk,
                MIN_NOMINATOR_BOND,
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

    // 4. wait for era and payout; repeat
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
