use std::{iter, time::Instant};

use aleph_client::{
    balances_batch_transfer, keypair_from_string, payout_stakers_and_assert_locked_balance,
    staking_batch_bond, staking_batch_nominate, staking_bond, staking_validate, wait_for_next_era,
    AnyConnection, RootConnection, SignedConnection,
};
use clap::{ArgGroup, Parser};
use log::{info, trace, warn};
use primitives::{
    staking::{MAX_NOMINATORS_REWARDED_PER_VALIDATOR, MIN_NOMINATOR_BOND, MIN_VALIDATOR_BOND},
    TOKEN,
};
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use sp_core::{crypto::AccountId32, sr25519::Pair as KeyPair, Pair};
use sp_keyring::AccountKeyring;
use substrate_api_client::{extrinsic::staking::RewardDestination, AccountId, XtStatus};

// testcase parameters
const NOMINATOR_COUNT: u32 = MAX_NOMINATORS_REWARDED_PER_VALIDATOR;
const ERAS_TO_WAIT: u32 = 100;

// we need to schedule batches for limited call count, otherwise we'll exhaust a block max weight
const BOND_CALL_BATCH_LIMIT: usize = 256;
const NOMINATE_CALL_BATCH_LIMIT: usize = 220;
const TRANSFER_CALL_BATCH_LIMIT: usize = 1024;

#[derive(Debug, Parser, Clone)]
#[clap(version = "1.0")]
#[clap(group(ArgGroup::new("valid").required(true)))]
struct Config {
    /// WS endpoint address of the node to connect to. Use IP:port syntax, e.g. 127.0.0.1:9944
    #[clap(long, default_value = "127.0.0.1:9944")]
    pub address: String,

    /// A path to a file that contains the root account seed.
    /// If not given, Alice is assumed to be the root.
    #[clap(long)]
    pub root_seed_file: Option<String>,

    /// A path to a file that contains seeds of validators, each in a separate line.
    /// If not given, validators 0, 1, ..., validator_count are assumed
    /// Only valid if validator count is not provided.
    #[clap(long, group = "valid")]
    pub validators_seed_file: Option<String>,

    /// Number of testcase validators.
    /// Only valid if validator seed file is not provided.
    #[clap(long, group = "valid")]
    pub validator_count: Option<u32>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    info!("Running payout_stakers bench.");
    let start = Instant::now();

    let Config {
        address,
        root_seed_file,
        validators_seed_file,
        validator_count,
    } = Config::parse();

    let sudoer = get_sudoer_keypair(root_seed_file);

    let connection = RootConnection::new(&address, sudoer);

    let validators = match validators_seed_file {
        Some(validators_seed_file) => {
            let validators_seeds = std::fs::read_to_string(&validators_seed_file)
                .unwrap_or_else(|_| panic!("Failed to read file {}", validators_seed_file));
            validators_seeds
                .split('\n')
                .filter(|seed| !seed.is_empty())
                .map(keypair_from_string)
                .collect()
        }
        None => (0..validator_count.unwrap())
            .map(derive_user_account_from_numeric_seed)
            .collect::<Vec<_>>(),
    };
    let validator_count = validators.len() as u32;
    warn!("Make sure you have exactly {} nodes run in the background, otherwise you'll see extrinsic send failed errors.", validator_count);

    let controllers = generate_controllers_for_validators(validator_count);

    bond_validators_funds_and_choose_controllers(&address, controllers.clone(), validators.clone());
    send_validate_txs(&address, controllers);

    let validators_and_nominator_stashes =
        setup_test_validators_and_nominator_stashes(&connection, validators);

    wait_for_successive_eras(
        &address,
        &connection,
        validators_and_nominator_stashes,
        ERAS_TO_WAIT,
    )?;

    let elapsed = Instant::now().duration_since(start);
    println!("Ok! Elapsed time {}ms", elapsed.as_millis());

    Ok(())
}

/// Get key pair based on seed file or default when seed file is not provided.
fn get_sudoer_keypair(root_seed_file: Option<String>) -> KeyPair {
    match root_seed_file {
        Some(root_seed_file) => {
            let root_seed = std::fs::read_to_string(&root_seed_file)
                .unwrap_or_else(|_| panic!("Failed to read file {}", root_seed_file));
            keypair_from_string(root_seed.trim())
        }
        None => AccountKeyring::Alice.pair(),
    }
}

/// For a given set of validators, generates key pairs for the corresponding controllers.
fn generate_controllers_for_validators(validator_count: u32) -> Vec<KeyPair> {
    (0..validator_count)
        .map(|seed| keypair_from_string(&format!("//{}//Controller", seed)))
        .collect::<Vec<_>>()
}

/// For a given set of validators, generates nominator accounts (controllers and stashes).
/// Bonds nominator controllers to the corresponding nominator stashes.
fn setup_test_validators_and_nominator_stashes(
    connection: &RootConnection,
    validators: Vec<KeyPair>,
) -> Vec<(KeyPair, Vec<AccountId32>)> {
    validators
        .iter()
        .enumerate()
        .map(|(validator_index, validator)| {
            let (nominator_controller_accounts, nominator_stash_accounts) =
                generate_nominator_accounts_with_minimal_bond(
                    &connection.as_signed(),
                    validator_index as u32,
                    validators.len() as u32,
                );
            let nominee_account = AccountId::from(validator.public());
            info!("Nominating validator {}", nominee_account);
            nominate_validator(
                connection,
                nominator_controller_accounts,
                nominator_stash_accounts.clone(),
                nominee_account,
            );
            (validator.clone(), nominator_stash_accounts)
        })
        .collect()
}

pub fn derive_user_account_from_numeric_seed(seed: u32) -> KeyPair {
    trace!("Generating account from numeric seed {}", seed);
    keypair_from_string(&format!("//{}", seed))
}

/// For a given number of eras, in each era check whether stash balances of a validator are locked.
fn wait_for_successive_eras<C: AnyConnection>(
    address: &str,
    connection: &C,
    validators_and_nominator_stashes: Vec<(KeyPair, Vec<AccountId>)>,
    eras_to_wait: u32,
) -> anyhow::Result<()> {
    // in order to have over 8k nominators we need to wait around 60 seconds all calls to be processed
    // that means not all 8k nominators we'll make i to era 1st, hence we need to wait to 2nd era
    wait_for_next_era(connection)?;
    wait_for_next_era(connection)?;
    // then we wait another full era to test rewards
    let mut current_era = wait_for_next_era(connection)?;
    for _ in 0..eras_to_wait {
        info!(
            "Era {} started, claiming rewards for era {}",
            current_era,
            current_era - 1
        );
        validators_and_nominator_stashes
            .iter()
            .for_each(|(validator, nominators_stashes)| {
                let validator_connection = SignedConnection::new(address, validator.clone());
                let validator_account = AccountId::from(validator.public());
                info!("Doing payout_stakers for validator {}", validator_account);
                payout_stakers_and_assert_locked_balance(
                    &validator_connection,
                    &[&nominators_stashes[..], &[validator_account.clone()]].concat(),
                    &validator_account,
                    current_era,
                );
            });
        current_era = wait_for_next_era(connection)?;
    }
    Ok(())
}

/// Nominates a specific validator based on the nominator controller and stash accounts.
fn nominate_validator(
    connection: &RootConnection,
    nominator_controller_accounts: Vec<AccountId>,
    nominator_stash_accounts: Vec<AccountId>,
    nominee_account: AccountId,
) {
    let stash_controller_accounts = nominator_stash_accounts
        .iter()
        .zip(nominator_controller_accounts.iter())
        .collect::<Vec<_>>();
    stash_controller_accounts
        .chunks(BOND_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            let mut rng = thread_rng();
            staking_batch_bond(
                connection,
                chunk,
                (rng.gen::<u128>() % 100) * TOKEN + MIN_NOMINATOR_BOND,
                RewardDestination::Staked,
            )
        });
    let nominator_nominee_accounts = nominator_controller_accounts
        .iter()
        .zip(iter::repeat(&nominee_account))
        .collect::<Vec<_>>();
    nominator_nominee_accounts
        .chunks(NOMINATE_CALL_BATCH_LIMIT)
        .for_each(|chunk| staking_batch_nominate(connection, chunk));
}

/// Bonds the funds of the validators.
/// Chooses controller accounts for the corresponding validators.
/// We assume stash == validator != controller.
fn bond_validators_funds_and_choose_controllers(
    address: &str,
    controllers: Vec<KeyPair>,
    validators: Vec<KeyPair>,
) {
    controllers
        .into_par_iter()
        .zip(validators.into_par_iter())
        .for_each(|(controller, validator)| {
            let connection = SignedConnection::new(address, validator);
            let controller_account_id = AccountId::from(controller.public());
            staking_bond(
                &connection,
                MIN_VALIDATOR_BOND,
                &controller_account_id,
                XtStatus::InBlock,
            );
        });
}

/// Submits candidate validators via controller accounts.
/// We assume stash == validator != controller.
fn send_validate_txs(address: &str, controllers: Vec<KeyPair>) {
    controllers.par_iter().for_each(|controller| {
        let mut rng = thread_rng();
        let connection = SignedConnection::new(address, controller.clone());
        staking_validate(&connection, rng.gen::<u8>() % 100, XtStatus::InBlock);
    });
}

/// For a specific validator given by index, generates a predetermined number of nominator accounts.
/// Nominator accounts are produced as (controller, stash) tuples with initial endowments.
fn generate_nominator_accounts_with_minimal_bond(
    connection: &SignedConnection,
    validator_number: u32,
    validators_count: u32,
) -> (Vec<AccountId>, Vec<AccountId>) {
    info!(
        "Generating nominator accounts for validator {}",
        validator_number
    );
    let mut controller_accounts = vec![];
    let mut stash_accounts = vec![];
    (0..NOMINATOR_COUNT).for_each(|nominator_number| {
        let idx = validators_count + nominator_number + NOMINATOR_COUNT * validator_number;
        let controller = keypair_from_string(&format!("//{}//Controller", idx));
        let stash = keypair_from_string(&format!("//{}//Stash", idx));
        controller_accounts.push(AccountId::from(controller.public()));
        stash_accounts.push(AccountId::from(stash.public()));
    });
    controller_accounts
        .chunks(TRANSFER_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            balances_batch_transfer(connection, chunk.to_vec(), TOKEN);
        });
    stash_accounts
        .chunks(TRANSFER_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            balances_batch_transfer(connection, chunk.to_vec(), MIN_NOMINATOR_BOND * 10);
            // potentially change to + 1
        });

    (controller_accounts, stash_accounts)
}
